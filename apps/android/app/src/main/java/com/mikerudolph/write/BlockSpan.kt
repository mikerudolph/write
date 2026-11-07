package com.mikerudolph.write

import android.content.res.Resources
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.Typeface
import android.graphics.drawable.BitmapDrawable
import android.graphics.drawable.Drawable
import android.text.TextPaint
import android.text.style.ImageSpan
import android.text.style.MetricAffectingSpan
import uniffi.write_ffi.BlockImage

class BlockSpan(
    val blockID: String,
    val contentKey: String,
    drawable: Drawable,
) : ImageSpan(drawable, ALIGN_BOTTOM)

class HeadingSpan(level: UByte) : MetricAffectingSpan() {
    private val scale = when (level.toInt()) {
        1 -> 1.5f
        2 -> 1.25f
        else -> 1.1f
    }

    private fun apply(paint: TextPaint) {
        paint.textSize *= scale
        paint.typeface = Typeface.create(Typeface.DEFAULT, Typeface.BOLD)
    }

    override fun updateMeasureState(paint: TextPaint) = apply(paint)

    override fun updateDrawState(paint: TextPaint) = apply(paint)
}

class BlockPicture(
    val bitmap: Bitmap,
    private val naturalWidth: Float,
    private val naturalHeight: Float,
    val failure: String?,
) {
    fun drawable(resources: Resources, available: Int): Drawable {
        val density = resources.displayMetrics.density
        val width = naturalWidth * density
        val fit = minOf(1f, available / maxOf(width, 1f))
        return BitmapDrawable(resources, bitmap).apply {
            setBounds(0, 0, (width * fit).toInt(), (naturalHeight * density * fit).toInt())
        }
    }

    companion object {
        private const val MAX_PIXELS = 2048

        fun decode(block: BlockImage): BlockPicture? {
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(block.bytes, 0, block.bytes.size, bounds)
            var sample = 1
            while (maxOf(bounds.outWidth, bounds.outHeight) / sample > MAX_PIXELS) sample *= 2
            val options = BitmapFactory.Options().apply { inSampleSize = sample }
            val bitmap = BitmapFactory.decodeByteArray(block.bytes, 0, block.bytes.size, options) ?: return null
            return BlockPicture(bitmap, block.width, block.height, block.failure)
        }
    }
}
