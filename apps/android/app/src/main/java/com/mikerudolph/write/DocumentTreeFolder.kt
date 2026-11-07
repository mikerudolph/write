package com.mikerudolph.write

import android.content.ContentResolver
import android.net.Uri
import android.provider.DocumentsContract
import android.provider.DocumentsContract.Document
import java.util.concurrent.ConcurrentHashMap
import uniffi.write_ffi.StorageFolder
import uniffi.write_ffi.StoreFailure

class DocumentTreeFolder(
    private val resolver: ContentResolver,
    private val tree: Uri,
) : StorageFolder {
    private val known = ConcurrentHashMap<String, String>()

    private class Child(val id: String, val name: String, val isDirectory: Boolean)

    private fun <T> guarded(action: () -> T): T = try {
        action()
    } catch (failure: StoreFailure) {
        throw failure
    } catch (failure: Exception) {
        throw StoreFailure.Failure(failure.message ?: "The storage location failed")
    }

    private fun uriOf(id: String): Uri = DocumentsContract.buildDocumentUriUsingTree(tree, id)

    private fun children(parent: String): List<Child> {
        val uri = DocumentsContract.buildChildDocumentsUriUsingTree(tree, parent)
        val columns = arrayOf(Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE)
        val output = mutableListOf<Child>()
        resolver.query(uri, columns, null, null, null)?.use { cursor ->
            while (cursor.moveToNext()) {
                output += Child(
                    cursor.getString(0),
                    cursor.getString(1),
                    cursor.getString(2) == Document.MIME_TYPE_DIR,
                )
            }
        }
        return output
    }

    private fun directory(parent: String, name: String): String =
        children(parent).firstOrNull { it.isDirectory && it.name == name }?.id
            ?: DocumentsContract.getDocumentId(
                DocumentsContract.createDocument(resolver, uriOf(parent), Document.MIME_TYPE_DIR, name)
                    ?: throw StoreFailure.Failure("The storage location refused a new directory"),
            )

    private fun root(): String = directory(DocumentsContract.getTreeDocumentId(tree), LIBRARY_DIRECTORY)

    private fun walk(parent: String, prefix: String, output: MutableList<String>) {
        for (child in children(parent)) {
            val path = prefix + child.name
            if (child.isDirectory) {
                walk(child.id, "$path/", output)
            } else if (!child.name.endsWith(PARTIAL)) {
                known[path] = child.id
                output += path
            }
        }
    }

    override fun list(): List<String> = guarded {
        val output = mutableListOf<String>()
        walk(root(), "", output)
        output
    }

    override fun read(path: String): ByteArray = guarded {
        val id = known[path] ?: throw StoreFailure.Failure("The file $path is not known")
        resolver.openInputStream(uriOf(id))?.use { it.readBytes() }
            ?: throw StoreFailure.Failure("The file $path did not open")
    }

    override fun write(path: String, bytes: ByteArray) = guarded {
        val parts = path.split('/')
        var parent = root()
        for (part in parts.dropLast(1)) {
            parent = directory(parent, part)
        }
        val name = parts.last()
        val temporaryName = name + PARTIAL
        children(parent).filter { it.name == name || it.name == temporaryName }.forEach {
            DocumentsContract.deleteDocument(resolver, uriOf(it.id))
        }
        val created = DocumentsContract.createDocument(resolver, uriOf(parent), BINARY_TYPE, temporaryName)
            ?: throw StoreFailure.Failure("The storage location refused a new file")
        resolver.openOutputStream(created, "wt")?.use { it.write(bytes) }
            ?: throw StoreFailure.Failure("The file $path did not open for writing")
        val renamed = DocumentsContract.renameDocument(resolver, created, name)
            ?: throw StoreFailure.Failure("The storage location refused a rename")
        known[path] = DocumentsContract.getDocumentId(renamed)
    }

    override fun delete(path: String) = guarded {
        val id = known.remove(path) ?: return@guarded
        DocumentsContract.deleteDocument(resolver, uriOf(id))
        Unit
    }

    private companion object {
        const val LIBRARY_DIRECTORY = "Write Library"
        const val PARTIAL = ".partial"
        const val BINARY_TYPE = "application/octet-stream"
    }
}
