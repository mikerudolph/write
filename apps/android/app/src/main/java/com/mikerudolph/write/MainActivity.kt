package com.mikerudolph.write

import android.app.Activity
import android.content.Intent
import android.graphics.Typeface
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.text.Editable
import android.text.InputFilter
import android.text.InputType
import android.text.TextWatcher
import android.view.GestureDetector
import android.view.Gravity
import android.view.MotionEvent
import android.view.View
import android.view.WindowInsetsController
import android.view.inputmethod.InputMethodManager
import android.widget.AdapterView
import android.widget.ArrayAdapter
import android.widget.Button
import android.widget.EditText
import android.widget.ImageView
import android.widget.LinearLayout
import android.widget.ListView
import android.widget.TextView
import android.window.OnBackInvokedCallback
import android.window.OnBackInvokedDispatcher
import java.io.File
import uniffi.write_ffi.Document
import uniffi.write_ffi.DocumentLibrary
import uniffi.write_ffi.DocumentSummary
import uniffi.write_ffi.EmbedInfo
import uniffi.write_ffi.StoreFailure
import uniffi.write_ffi.TrashedDocument
import uniffi.write_ffi.blockTemplate
import uniffi.write_ffi.headingRanges
import uniffi.write_ffi.prepareRendering

class MainActivity : Activity() {
    private lateinit var library: DocumentLibrary
    private lateinit var listScreen: LinearLayout
    private lateinit var editorScreen: LinearLayout
    private lateinit var status: TextView
    private lateinit var editor: EditText
    private lateinit var adapter: ArrayAdapter<String>
    private lateinit var syncStatus: TextView
    private lateinit var overlayScreen: LinearLayout
    private lateinit var overlayTitle: TextView
    private lateinit var overlayBody: LinearLayout
    private lateinit var notice: TextView
    private val hideNotice = Runnable { notice.visibility = View.GONE }
    private var confirmingDelete: String? = null

    private val handler = Handler(Looper.getMainLooper())
    private val refreshStats = Runnable { showStats() }
    private val syncSoon = Runnable { syncNow() }
    private val syncTimer = object : Runnable {
        override fun run() {
            syncNow()
            handler.postDelayed(this, SYNC_INTERVAL_MS)
        }
    }
    private var isSyncing = false
    private var summaries: List<DocumentSummary> = emptyList()
    private var document: Document? = null
    private var isLoading = false
    private var needsReconcile = false
    private var loadGeneration = 0
    private var backCallback: Any? = null
    private val pictures = HashMap<String, BlockPicture>()
    private var openSource: EditText? = null
    private var openBlockID: String? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        library = DocumentLibrary.open(File(filesDir, "library.sqlite3").path)
        Thread { prepareRendering() }.start()
        guarded { library.purgeExpired() }
        buildViews()
        val openID = savedInstanceState?.getString(OPEN_DOCUMENT)
        if (openID == null) showList() else open(openID)
    }

    override fun onSaveInstanceState(outState: Bundle) {
        super.onSaveInstanceState(outState)
        document?.let { outState.putString(OPEN_DOCUMENT, it.id()) }
    }

    override fun onResume() {
        super.onResume()
        handler.post(syncTimer)
    }

    override fun onPause() {
        super.onPause()
        commit()
        handler.removeCallbacks(syncTimer)
        syncNow()
    }

    @Deprecated("The application uses one simple result path")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        @Suppress("DEPRECATION")
        super.onActivityResult(requestCode, resultCode, data)
        val uri = data?.data
        if (requestCode == CHOOSE_IMAGE && resultCode == RESULT_OK && uri != null) {
            addImage(uri)
        }
        if (requestCode == CHOOSE_FOLDER && resultCode == RESULT_OK && uri != null) {
            contentResolver.takePersistableUriPermission(
                uri,
                Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION,
            )
            guarded { library.setSyncLocation(uri.toString()) }
            syncNow()
        }
    }

    private fun chooseFolder() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).addFlags(
            Intent.FLAG_GRANT_READ_URI_PERMISSION or
                Intent.FLAG_GRANT_WRITE_URI_PERMISSION or
                Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION,
        )
        @Suppress("DEPRECATION")
        startActivityForResult(intent, CHOOSE_FOLDER)
    }

    private fun syncNow() {
        val location = guarded { library.syncLocation() } ?: run {
            syncStatus.text = "No storage location. Your writing stays on this device."
            return
        }
        if (isSyncing) return
        isSyncing = true
        commit()
        val folder = DocumentTreeFolder(contentResolver, Uri.parse(location))
        Thread {
            val result = runCatching { library.sync(folder) }
            runOnUiThread { finishSync(result) }
        }.start()
    }

    private fun finishSync(result: Result<uniffi.write_ffi.SyncReport>) {
        isSyncing = false
        result.onSuccess { report ->
            val current = document
            if (current != null && report.changed.contains(current.id())) {
                applyRemoteText(current)
                refreshBlocks()
                sendRemoteSourceToBlockEditor()
            }
            if (current != null && report.assetsReceived > 0uL) refreshBlocks()
            if (current != null && (report.removed.contains(current.id()) || current.isInTrash())) {
                document = null
                closeOverlay()
                showList()
            }
            if (report.removed.isNotEmpty() && document == null) refreshList()
            if (report.changed.isNotEmpty() && document == null) refreshList()
            showStats()
            syncStatus.text = report.failure?.let { "Sync failed: $it" }
                ?: "Synced: ${report.pulled} received, ${report.pushed} sent"
        }.onFailure {
            syncStatus.text = "Sync failed: ${it.message}"
        }
    }

    private fun applyRemoteText(current: Document) {
        val local = editor.text.toString()
        val remote = current.text()
        if (local == remote) return
        var prefix = local.commonPrefixWith(remote).length
        if (prefix > 0 && Character.isHighSurrogate(remote[prefix - 1])) prefix -= 1
        var suffix = local.substring(prefix).commonSuffixWith(remote.substring(prefix)).length
        if (suffix > 0 && Character.isLowSurrogate(remote[remote.length - suffix])) suffix -= 1
        isLoading = true
        editor.text.replace(prefix, local.length - suffix, remote.substring(prefix, remote.length - suffix))
        styleHeadings(prefix, remote.length - suffix)
        isLoading = false
    }

    private fun goBack() {
        if (overlayScreen.visibility == View.VISIBLE) closeOverlay() else showList()
    }

    @Deprecated("The application uses the Activity back method")
    override fun onBackPressed() {
        if (editorScreen.visibility == View.VISIBLE || overlayScreen.visibility == View.VISIBLE) {
            goBack()
        } else {
            @Suppress("DEPRECATION")
            super.onBackPressed()
        }
    }

    private fun setEditorBack(enabled: Boolean) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val current = backCallback as OnBackInvokedCallback?
        if (enabled && current == null) {
            val callback = OnBackInvokedCallback { goBack() }
            onBackInvokedDispatcher.registerOnBackInvokedCallback(
                OnBackInvokedDispatcher.PRIORITY_DEFAULT,
                callback,
            )
            backCallback = callback
        } else if (!enabled && current != null) {
            onBackInvokedDispatcher.unregisterOnBackInvokedCallback(current)
            backCallback = null
        }
    }

    private fun buildViews() {
        adapter = ArrayAdapter(this, android.R.layout.simple_list_item_1, mutableListOf<String>())
        val list = ListView(this).apply {
            adapter = this@MainActivity.adapter
            onItemClickListener = AdapterView.OnItemClickListener { _, _, position, _ ->
                open(summaries[position].id)
            }
            onItemLongClickListener = AdapterView.OnItemLongClickListener { _, _, position, _ ->
                moveToTrash(summaries[position])
                true
            }
        }
        val newButton = plainButton("New document") { createDocument() }
        val folderButton = plainButton("Storage location") { chooseFolder() }
        val syncButton = plainButton("Sync now") { syncNow() }
        val trashButton = plainButton("Trash") { showTrash() }
        syncStatus = TextView(this)
        listScreen = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(newButton)
            addView(folderButton)
            addView(syncButton)
            addView(trashButton)
            addView(syncStatus)
            addView(list, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        }

        status = TextView(this).apply {
            setTextColor(MUTED_COLOR)
            textSize = STATUS_SIZE_SP
            setPadding(dp(COLUMN_MARGIN_DP), dp(4), dp(COLUMN_MARGIN_DP), 0)
        }
        val blockButtons = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            addView(
                plainButton("☰") { showList() },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 0.7f),
            )
            for ((label, kind) in listOf("Table" to KIND_TABLE, "Math" to KIND_MATH, "Diagram" to KIND_DIAGRAM)) {
                addView(
                    plainButton(label) { insertBlock(kind, blockTemplate(kind), true) },
                    LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
                )
            }
            addView(
                plainButton("Image") { chooseImage() },
                LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f),
            )
        }
        val blockTaps = GestureDetector(
            this,
            object : GestureDetector.SimpleOnGestureListener() {
                override fun onSingleTapUp(event: MotionEvent): Boolean {
                    blockAt(event)?.let { id -> handler.post { openBlock(id) } }
                    return false
                }
            },
        )
        editor = EditText(this).apply {
            id = R.id.editor
            gravity = Gravity.TOP or Gravity.START
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_CAP_SENTENCES
            background = null
            setBackgroundColor(BACKGROUND_COLOR)
            setTextColor(TEXT_COLOR)
            textSize = BODY_SIZE_SP
            setLineSpacing(0f, LINE_SPACING)
            val side = dp(COLUMN_MARGIN_DP)
            setPadding(side, dp(COLUMN_TOP_DP), side, side)
            filters = arrayOf(InputFilter { source, start, end, _, _, _ -> withoutAnchors(source, start, end) })
            addTextChangedListener(EditRecorder())
            setOnTouchListener { _, event ->
                blockTaps.onTouchEvent(event)
                false
            }
        }
        editorScreen = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            addView(status)
            addView(blockButtons)
            addView(editor, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
        }

        overlayTitle = TextView(this).apply {
            setTextColor(TEXT_COLOR)
            textSize = OVERLAY_TITLE_SP
            typeface = Typeface.DEFAULT_BOLD
        }
        val overlayHeader = LinearLayout(this).apply {
            orientation = LinearLayout.HORIZONTAL
            gravity = Gravity.CENTER_VERTICAL
            setPadding(dp(COLUMN_MARGIN_DP), dp(COLUMN_TOP_DP), 0, 0)
            addView(overlayTitle, LinearLayout.LayoutParams(0, LinearLayout.LayoutParams.WRAP_CONTENT, 1f))
            addView(plainButton("✕") { closeOverlay() })
        }
        overlayBody = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(dp(COLUMN_MARGIN_DP), dp(COLUMN_TOP_DP), dp(COLUMN_MARGIN_DP), dp(COLUMN_MARGIN_DP))
        }
        overlayScreen = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            visibility = View.GONE
            addView(overlayHeader)
            addView(
                android.widget.ScrollView(this@MainActivity).apply { addView(overlayBody) },
                LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f),
            )
        }
        notice = TextView(this).apply {
            setTextColor(FAILURE_COLOR)
            setPadding(dp(COLUMN_MARGIN_DP), dp(4), dp(COLUMN_MARGIN_DP), dp(4))
            visibility = View.GONE
        }

        val root = LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            fitsSystemWindows = true
            setBackgroundColor(BACKGROUND_COLOR)
            addView(listScreen, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(editorScreen, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(overlayScreen, LinearLayout.LayoutParams(LinearLayout.LayoutParams.MATCH_PARENT, 0, 1f))
            addView(notice)
        }
        setContentView(root)
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
            val lightBars = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS
            window.insetsController?.setSystemBarsAppearance(lightBars, lightBars)
        }
    }

    private fun dp(value: Int): Int = (value * resources.displayMetrics.density).toInt()

    private fun plainButton(label: String, action: () -> Unit): Button =
        Button(this, null, android.R.attr.borderlessButtonStyle).apply {
            text = label
            isAllCaps = false
            setTextColor(MUTED_COLOR)
            setOnClickListener { action() }
        }

    private fun styleHeadings(start: Int, end: Int) {
        val text = editor.text
        val from = if (start <= 0) 0 else text.lastIndexOf('\n', start - 1) + 1
        val newline = text.indexOf('\n', minOf(end, text.length))
        val to = if (newline < 0) text.length else newline
        for (span in text.getSpans(from, to, HeadingSpan::class.java)) {
            if (text.getSpanStart(span) >= from) text.removeSpan(span)
        }
        for (heading in headingRanges(text.substring(from, to))) {
            text.setSpan(
                HeadingSpan(heading.level),
                from + heading.start.toInt(),
                from + heading.end.toInt(),
                android.text.Spanned.SPAN_EXCLUSIVE_INCLUSIVE,
            )
        }
    }

    private fun showList() {
        commit()
        loadGeneration += 1
        document = null
        refreshList()
        editorScreen.visibility = View.GONE
        listScreen.visibility = View.VISIBLE
        setEditorBack(false)
    }

    private fun refreshList() {
        summaries = guarded { library.list() } ?: emptyList()
        adapter.clear()
        adapter.addAll(summaries.map { it.title })
    }

    private fun createDocument() {
        guarded { library.create() }?.let { show(it) }
    }

    private fun open(id: String) {
        loadGeneration += 1
        val generation = loadGeneration
        Thread {
            val result = runCatching { library.openDocument(id) }
            val rendered = result.getOrNull()?.embeds().orEmpty().map { contentKey(it) to render(it) }
            runOnUiThread {
                if (generation == loadGeneration) {
                    pictures.clear()
                    rendered.forEach { (key, picture) -> if (picture?.failure == null && picture != null) pictures[key] = picture }
                    result.onSuccess { show(it) }.onFailure { report(it) }
                }
            }
        }.start()
    }

    private fun show(opened: Document) {
        isLoading = true
        editor.setText(opened.text())
        editor.setSelection(editor.length())
        document = opened
        refreshBlocks()
        styleHeadings(0, editor.length())
        isLoading = false
        listScreen.visibility = View.GONE
        editorScreen.visibility = View.VISIBLE
        setEditorBack(true)
        editor.requestFocus()
        showStats()
    }

    private fun showOverlay(title: String, body: LinearLayout.() -> Unit) {
        overlayTitle.text = title
        overlayBody.removeAllViews()
        overlayBody.body()
        listScreen.visibility = View.GONE
        editorScreen.visibility = View.GONE
        overlayScreen.visibility = View.VISIBLE
        setEditorBack(true)
    }

    private fun closeOverlay() {
        if (overlayScreen.visibility != View.VISIBLE) return
        overlayScreen.visibility = View.GONE
        openSource = null
        openBlockID = null
        confirmingDelete = null
        if (document != null) {
            editorScreen.visibility = View.VISIBLE
            refreshBlocks()
            editor.requestFocus()
        } else {
            listScreen.visibility = View.VISIBLE
            setEditorBack(false)
            refreshList()
        }
    }

    private fun moveToTrash(summary: DocumentSummary) {
        guarded { library.moveToTrash(summary.id) }
        refreshList()
        syncNow()
    }

    private fun showTrash() {
        val items = guarded { library.listTrash() } ?: return
        showOverlay("Trash") {
            addView(
                TextView(this@MainActivity).apply {
                    text = "Write removes documents from Trash after 30 days."
                    setTextColor(MUTED_COLOR)
                },
            )
            if (items.isEmpty()) {
                addView(
                    TextView(this@MainActivity).apply {
                        text = "Trash is empty."
                        setTextColor(MUTED_COLOR)
                        setPadding(0, dp(COLUMN_TOP_DP), 0, 0)
                    },
                )
            }
            items.forEach { addView(trashRow(it)) }
        }
    }

    private fun trashRow(item: TrashedDocument): LinearLayout {
        val isConfirming = confirmingDelete == item.id
        return LinearLayout(this).apply {
            orientation = LinearLayout.VERTICAL
            setPadding(0, dp(COLUMN_TOP_DP), 0, 0)
            addView(
                TextView(this@MainActivity).apply {
                    text = if (isConfirming) "Delete from all devices? You cannot undo this." else item.title
                    setTextColor(TEXT_COLOR)
                },
            )
            val actions = LinearLayout(this@MainActivity).apply { orientation = LinearLayout.HORIZONTAL }
            if (isConfirming) {
                actions.addView(plainButton("Delete forever") { purge(item) })
                actions.addView(
                    plainButton("Keep") {
                        confirmingDelete = null
                        showTrash()
                    },
                )
            } else {
                actions.addView(plainButton("Restore") { restore(item) })
                actions.addView(
                    plainButton("Delete") {
                        confirmingDelete = item.id
                        showTrash()
                    },
                )
            }
            addView(actions)
        }
    }

    private fun restore(item: TrashedDocument) {
        guarded { library.restoreDocument(item.id) }
        syncNow()
        showTrash()
    }

    private fun purge(item: TrashedDocument) {
        confirmingDelete = null
        guarded { library.purgeDocument(item.id) }
        syncNow()
        showTrash()
    }

    private fun withoutAnchors(source: CharSequence, start: Int, end: Int): CharSequence? {
        if (isLoading || !source.subSequence(start, end).contains(BLOCK_ANCHOR)) return null
        return source.subSequence(start, end).toString().replace(BLOCK_ANCHOR.toString(), "")
    }

    private fun contentKey(embed: EmbedInfo): String = "${embed.kind}|${embed.source}"

    private fun render(embed: EmbedInfo): BlockPicture? {
        val scale = maxOf(resources.displayMetrics.density, 2f)
        return BlockPicture.decode(library.blockImage(embed.kind, embed.source, false, scale))
    }

    private fun picture(embed: EmbedInfo): BlockPicture? {
        val key = contentKey(embed)
        pictures[key]?.let { return it }
        return render(embed)?.also { if (it.failure == null) pictures[key] = it }
    }

    private fun blockWidth(): Int {
        val width = if (editor.width > 0) editor.width else resources.displayMetrics.widthPixels
        return width - editor.totalPaddingLeft - editor.totalPaddingRight
    }

    private fun refreshBlocks() {
        val current = document ?: return
        val text = editor.text
        val embeds = current.embeds()
        val wanted = embeds.associate { it.position.toInt() to contentKey(it) }
        val present = HashSet<Int>()
        for (span in text.getSpans(0, text.length, BlockSpan::class.java)) {
            val position = text.getSpanStart(span)
            if (wanted[position] == span.contentKey && pictures.containsKey(span.contentKey)) {
                present += position
            } else {
                text.removeSpan(span)
            }
        }
        for (embed in embeds) {
            val position = embed.position.toInt()
            if (position in present || position >= text.length) continue
            val picture = picture(embed) ?: continue
            val span = BlockSpan(embed.id, contentKey(embed), picture.drawable(resources, blockWidth()))
            text.setSpan(span, position, position + 1, android.text.Spanned.SPAN_EXCLUSIVE_EXCLUSIVE)
        }
    }

    private fun insertBlock(kind: String, source: String, opensEditor: Boolean) {
        val current = document ?: return
        commit()
        val text = editor.text
        val start = minOf(editor.selectionStart, editor.selectionEnd).coerceAtLeast(0)
        val end = maxOf(editor.selectionStart, editor.selectionEnd).coerceAtLeast(0)
        val before = if (start > 0 && text[start - 1] != '\n') "\n" else ""
        val after = if (end == text.length || text[end] != '\n') "\n" else ""
        val inserted = before + BLOCK_ANCHOR + after
        val anchor = start + before.length
        val id = guarded {
            val id = current.createEmbed(kind, source)
            current.edit(start.toULong(), (end - start).toULong(), inserted)
            current.attachEmbed(anchor.toULong(), id)
            id
        } ?: return
        isLoading = true
        text.replace(start, end, inserted)
        isLoading = false
        editor.setSelection(start + inserted.length)
        refreshBlocks()
        showStats()
        handler.removeCallbacks(syncSoon)
        handler.postDelayed(syncSoon, SYNC_DELAY_MS)
        if (opensEditor) openBlock(id)
    }

    private fun chooseImage() {
        val intent = Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("image/*")
        @Suppress("DEPRECATION")
        startActivityForResult(intent, CHOOSE_IMAGE)
    }

    private fun addImage(uri: Uri) {
        Thread {
            val result = runCatching {
                val bytes = contentResolver.openInputStream(uri)?.use { it.readBytes() }
                    ?: throw StoreFailure.Failure("The image did not open")
                library.addImage(bytes)
            }
            runOnUiThread {
                result.onSuccess { insertBlock(KIND_IMAGE, it, false) }.onFailure { report(it) }
            }
        }.start()
    }

    private fun blockAt(event: MotionEvent): String? {
        val layout = editor.layout ?: return null
        val x = event.x.toInt() - editor.totalPaddingLeft + editor.scrollX
        val y = event.y.toInt() - editor.totalPaddingTop + editor.scrollY
        if (y < 0 || y >= layout.height) return null
        val line = layout.getLineForVertical(y)
        if (x > layout.getLineWidth(line)) return null
        val spans = editor.text.getSpans(layout.getLineStart(line), layout.getLineEnd(line), BlockSpan::class.java)
        return spans.firstOrNull()?.blockID
    }

    private fun openBlock(blockID: String) {
        val embed = document?.embeds()?.firstOrNull { it.id == blockID } ?: return
        val preview = ImageView(this).apply {
            adjustViewBounds = true
            maxHeight = dp(PREVIEW_MAX_DP)
        }
        val failure = TextView(this).apply { setTextColor(FAILURE_COLOR) }
        fun showPicture(picture: BlockPicture?) {
            preview.setImageBitmap(picture?.bitmap)
            failure.text = picture?.failure.orEmpty()
        }
        val source = EditText(this).apply {
            id = R.id.block_source
            setBackgroundColor(SOURCE_COLOR)
            setTextColor(TEXT_COLOR)
            setPadding(dp(12), dp(12), dp(12), dp(12))
            typeface = Typeface.MONOSPACE
            inputType = InputType.TYPE_CLASS_TEXT or
                InputType.TYPE_TEXT_FLAG_MULTI_LINE or
                InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
            setText(embed.source)
            setSelection(length())
            visibility = if (embed.kind == KIND_IMAGE) View.GONE else View.VISIBLE
        }
        source.addTextChangedListener(object : TextWatcher {
            override fun beforeTextChanged(text: CharSequence?, start: Int, count: Int, after: Int) = Unit
            override fun onTextChanged(text: CharSequence?, start: Int, before: Int, count: Int) = Unit
            override fun afterTextChanged(text: Editable?) {
                val value = text?.toString() ?: return
                guarded { document?.setEmbedSource(embed.id, value) }
                showPicture(picture(EmbedInfo(embed.id, embed.kind, value, embed.position)))
                handler.removeCallbacks(syncSoon)
                handler.postDelayed(syncSoon, SYNC_DELAY_MS)
            }
        })
        showPicture(picture(embed))
        openSource = source
        openBlockID = embed.id
        val title = when (embed.kind) {
            KIND_TABLE -> "Table"
            KIND_MATH -> "Math block"
            KIND_DIAGRAM -> "Diagram"
            else -> "Image"
        }
        showOverlay(title) {
            addView(source)
            addView(preview)
            addView(failure)
            addView(
                plainButton("Delete block") {
                    deleteBlock(embed.id)
                    closeOverlay()
                },
            )
        }
        if (embed.kind != KIND_IMAGE) {
            source.requestFocus()
            getSystemService(InputMethodManager::class.java).showSoftInput(source, 0)
        }
    }

    private fun deleteBlock(id: String) {
        val embed = document?.embeds()?.firstOrNull { it.id == id } ?: return
        val text = editor.text
        val start = embed.position.toInt()
        val end = if (start + 1 < text.length && text[start + 1] == '\n') start + 2 else start + 1
        text.delete(start, end)
    }

    private fun sendRemoteSourceToBlockEditor() {
        val source = openSource ?: return
        val embed = document?.embeds()?.firstOrNull { it.id == openBlockID } ?: return
        if (source.text.toString() != embed.source) source.setText(embed.source)
    }

    private fun commit() {
        val current = document ?: return
        if (guarded { current.reconcile(editor.text.toString()) } != null) {
            needsReconcile = false
        }
    }

    private fun showStats() {
        if (needsReconcile) commit()
        val stats = document?.stats() ?: return
        status.text = "${stats.words} words, ${stats.characters} characters"
    }

    private fun <T> guarded(action: () -> T): T? = try {
        action()
    } catch (failure: StoreFailure) {
        report(failure)
        null
    }

    private fun report(failure: Throwable) {
        notice.text = failure.message
        notice.visibility = View.VISIBLE
        handler.removeCallbacks(hideNotice)
        handler.postDelayed(hideNotice, NOTICE_MS)
    }

    private inner class EditRecorder : TextWatcher {
        private var styleStart = -1
        private var styleEnd = -1

        override fun beforeTextChanged(text: CharSequence?, start: Int, count: Int, after: Int) = Unit

        override fun onTextChanged(text: CharSequence?, start: Int, before: Int, count: Int) {
            val current = document
            if (isLoading || current == null || text == null) return
            styleStart = start
            styleEnd = start + count
            try {
                val inserted = text.subSequence(start, start + count).toString()
                current.edit(start.toULong(), before.toULong(), inserted)
            } catch (failure: StoreFailure) {
                needsReconcile = true
            }
        }

        override fun afterTextChanged(text: Editable?) {
            if (!isLoading && styleStart >= 0) {
                styleHeadings(styleStart, styleEnd)
                styleStart = -1
            }
            handler.removeCallbacks(refreshStats)
            handler.postDelayed(refreshStats, STATS_DELAY_MS)
            if (!isLoading) {
                handler.removeCallbacks(syncSoon)
                handler.postDelayed(syncSoon, SYNC_DELAY_MS)
            }
        }
    }

    private companion object {
        const val OPEN_DOCUMENT = "open_document"
        const val STATS_DELAY_MS = 200L
        const val SYNC_DELAY_MS = 3000L
        const val SYNC_INTERVAL_MS = 15000L
        const val CHOOSE_FOLDER = 1
        const val CHOOSE_IMAGE = 2
        const val BLOCK_ANCHOR = '\uFFFC'
        const val KIND_TABLE = "table"
        const val KIND_MATH = "math"
        const val KIND_DIAGRAM = "diagram"
        const val KIND_IMAGE = "image"
        const val FAILURE_COLOR = 0xFFC5221F.toInt()
        const val BACKGROUND_COLOR = 0xFFF9F9F9.toInt()
        const val TEXT_COLOR = 0xFF4A4F57.toInt()
        const val MUTED_COLOR = 0xFF868B93.toInt()
        const val BODY_SIZE_SP = 17f
        const val STATUS_SIZE_SP = 12f
        const val LINE_SPACING = 1.4f
        const val COLUMN_MARGIN_DP = 24
        const val COLUMN_TOP_DP = 16
        const val SOURCE_COLOR = 0xFFF2F4F5.toInt()
        const val OVERLAY_TITLE_SP = 24f
        const val NOTICE_MS = 6000L
        const val PREVIEW_MAX_DP = 320
    }
}
