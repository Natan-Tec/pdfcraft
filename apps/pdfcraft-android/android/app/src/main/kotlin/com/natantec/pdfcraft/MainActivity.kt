package com.natantec.pdfcraft

import android.app.NativeActivity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.os.FileObserver
import android.provider.OpenableColumns
import android.widget.Toast
import java.io.File

/**
 * Hosts the Rust app (libpdfcraft_android.so) and does what only Java APIs can do.
 *
 * It talks to Rust through files in `filesDir/mailbox`, so neither side needs JNI:
 *  - `requests/<id>.req`: Rust asks for a file picker (lines `kind=`, `title=`, `name=`, `extensions=`).
 *  - `results/<id>.res`: this side answers with one path per line (empty = cancelled).
 *  - `incoming/<n>.txt`: a file another app opened with PdfCraft; the line is the path of a copy.
 */
class MainActivity : NativeActivity() {
    private lateinit var mailbox: File
    private var observer: FileObserver? = null
    private var pendingId: String? = null
    private var pendingKind: String = ""
    private val handled = HashSet<String>()

    override fun onCreate(savedInstanceState: Bundle?) {
        mailbox = File(filesDir, "mailbox")
        for (d in listOf("requests", "results", "incoming")) File(mailbox, d).mkdirs()
        // A request left by a previous run can never be answered.
        File(mailbox, "requests").listFiles()?.forEach { it.delete() }
        super.onCreate(savedInstanceState)
        watchRequests()
        handleIntent(intent)
    }

    override fun onNewIntent(intent: Intent?) {
        super.onNewIntent(intent)
        handleIntent(intent)
    }

    override fun onDestroy() {
        observer?.stopWatching()
        super.onDestroy()
    }

    // ---- requests from Rust -------------------------------------------------------------

    private fun watchRequests() {
        val dir = File(mailbox, "requests")
        val obs = object : FileObserver(dir.path, MOVED_TO or CLOSE_WRITE) {
            override fun onEvent(event: Int, path: String?) {
                if (path != null && path.endsWith(".req")) runOnUiThread { handleRequest(File(dir, path)) }
            }
        }
        obs.startWatching()
        observer = obs
    }

    private fun handleRequest(file: File) {
        val id = file.name.removeSuffix(".req")
        if (!handled.add(id)) return
        val fields = try {
            file.readLines().mapNotNull { l -> l.indexOf('=').takeIf { it > 0 }?.let { l.substring(0, it) to l.substring(it + 1) } }.toMap()
        } catch (e: Exception) {
            answer(id, emptyList()); return
        }
        file.delete()
        val kind = fields["kind"] ?: "open"
        when (kind) {
            "save" -> {
                // No system dialog yet: documents are saved into the app's Documents folder.
                val dir = getExternalFilesDir("Documents") ?: filesDir
                dir.mkdirs()
                val name = safeName(fields["name"].orEmpty().ifBlank { "document.pdf" })
                Toast.makeText(this, "Saving to ${dir.path}", Toast.LENGTH_LONG).show()
                answer(id, listOf(File(dir, name).path))
            }
            "folder" -> {
                Toast.makeText(this, "Choosing folders is not supported on Android yet", Toast.LENGTH_SHORT).show()
                answer(id, emptyList())
            }
            else -> {
                if (pendingId != null) { answer(id, emptyList()); return }
                val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    type = "*/*"
                    putExtra(Intent.EXTRA_MIME_TYPES, mimeTypes(fields["extensions"].orEmpty()))
                    putExtra(Intent.EXTRA_ALLOW_MULTIPLE, kind == "open-many")
                }
                pendingId = id
                pendingKind = kind
                try {
                    @Suppress("DEPRECATION")
                    startActivityForResult(intent, PICK)
                } catch (e: Exception) {
                    pendingId = null
                    answer(id, emptyList())
                }
            }
        }
    }

    private fun mimeTypes(extensions: String): Array<String> {
        val map = mapOf(
            "pdf" to "application/pdf", "png" to "image/png", "jpg" to "image/jpeg", "jpeg" to "image/jpeg",
            "gif" to "image/gif", "bmp" to "image/bmp", "tif" to "image/tiff", "tiff" to "image/tiff",
            "txt" to "text/plain", "csv" to "text/csv", "xml" to "text/xml", "json" to "application/json",
        )
        val found = extensions.split(',').mapNotNull { map[it.trim().lowercase()] }.distinct()
        // Unknown types (certificates, fonts, ...) are not filtered, so nothing becomes unpickable.
        return if (found.isEmpty() || extensions.split(',').any { it.isNotBlank() && map[it.trim().lowercase()] == null }) arrayOf("*/*") else found.toTypedArray()
    }

    @Deprecated("Deprecated in Java")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode != PICK) return
        val id = pendingId ?: return
        pendingId = null
        val uris = ArrayList<Uri>()
        if (resultCode == RESULT_OK && data != null) {
            data.data?.let { uris.add(it) }
            data.clipData?.let { clip -> for (i in 0 until clip.itemCount) clip.getItemAt(i).uri?.let { if (!uris.contains(it)) uris.add(it) } }
        }
        if (uris.isEmpty()) { answer(id, emptyList()); return }
        Thread {
            val dir = File(cacheDir, "picked/$id").apply { mkdirs() }
            val paths = uris.mapNotNull { copyToCache(it, dir) }.map { it.path }
            answer(id, paths)
        }.start()
    }

    private fun answer(id: String, paths: List<String>) {
        val results = File(mailbox, "results")
        results.mkdirs()
        val tmp = File(results, "$id.tmp")
        tmp.writeText(paths.joinToString("\n"))
        tmp.renameTo(File(results, "$id.res"))
    }

    // ---- files other apps open with PdfCraft ------------------------------------------------

    private fun handleIntent(intent: Intent?) {
        intent ?: return
        val uri: Uri? = when (intent.action) {
            Intent.ACTION_VIEW -> intent.data
            Intent.ACTION_SEND -> @Suppress("DEPRECATION") intent.getParcelableExtra(Intent.EXTRA_STREAM)
            else -> null
        }
        uri ?: return
        Thread {
            val dir = File(cacheDir, "incoming/${System.nanoTime()}").apply { mkdirs() }
            val copy = copyToCache(uri, dir) ?: return@Thread
            val incoming = File(mailbox, "incoming").apply { mkdirs() }
            val tmp = File(incoming, "${System.nanoTime()}.tmp")
            tmp.writeText(copy.path)
            tmp.renameTo(File(incoming, tmp.name.removeSuffix(".tmp") + ".txt"))
        }.start()
    }

    // ---- helpers ------------------------------------------------------------------------------

    private fun copyToCache(uri: Uri, dir: File): File? {
        return try {
            var name = "document.pdf"
            contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { c ->
                if (c.moveToFirst()) c.getString(0)?.takeIf { it.isNotBlank() }?.let { name = it }
            } ?: uri.lastPathSegment?.let { name = it.substringAfterLast('/') }
            val out = File(dir, safeName(name))
            contentResolver.openInputStream(uri)?.use { input -> out.outputStream().use { input.copyTo(it) } } ?: return null
            out
        } catch (e: Exception) {
            null
        }
    }

    private fun safeName(name: String): String {
        val clean = name.replace(Regex("[\\\\/:*?\"<>|\\u0000-\\u001f]"), "_").trim().trimStart('.')
        return clean.ifEmpty { "document.pdf" }.take(120)
    }

    companion object {
        private const val PICK = 4711
    }
}
