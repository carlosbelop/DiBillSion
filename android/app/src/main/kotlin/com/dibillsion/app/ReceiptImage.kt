package com.dibillsion.app

import android.content.Context
import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.graphics.ImageDecoder
import android.net.Uri
import android.os.Build
import java.io.ByteArrayOutputStream
import java.io.IOException

/**
 * Receipt photos are shrunk on the phone before upload (PLAN §3.5): the long side to
 * [MAX_SIDE] px, JPEG at [QUALITY]. That keeps the text readable for the model and the
 * upload around 200–400 KB.
 */
object ReceiptImage {
    const val MAX_SIDE = 1600
    private const val QUALITY = 85

    /** The size to scale `width × height` to, never upscaling. */
    fun targetSize(width: Int, height: Int, maxSide: Int = MAX_SIDE): Pair<Int, Int> {
        val longest = maxOf(width, height)
        if (longest <= maxSide) return width to height
        // Integer math: round to the nearest pixel, at least 1.
        val w = maxOf(1, ((width.toLong() * maxSide + longest / 2) / longest).toInt())
        val h = maxOf(1, ((height.toLong() * maxSide + longest / 2) / longest).toInt())
        return w to h
    }

    /** JPEG bytes ready for `scanReceipt`, upright (EXIF rotation applied on API 28+). */
    @Throws(IOException::class)
    fun load(context: Context, uri: Uri): ByteArray {
        val bitmap = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            ImageDecoder.decodeBitmap(ImageDecoder.createSource(context.contentResolver, uri)) { decoder, info, _ ->
                val (w, h) = targetSize(info.size.width, info.size.height)
                decoder.setTargetSize(w, h)
                decoder.allocator = ImageDecoder.ALLOCATOR_SOFTWARE
            }
        } else {
            decodeLegacy(context, uri)
        }
        return ByteArrayOutputStream().use { out ->
            bitmap.compress(Bitmap.CompressFormat.JPEG, QUALITY, out)
            bitmap.recycle()
            out.toByteArray()
        }
    }

    private fun decodeLegacy(context: Context, uri: Uri): Bitmap {
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        context.contentResolver.openInputStream(uri)?.use { BitmapFactory.decodeStream(it, null, bounds) }
        var sample = 1
        while (maxOf(bounds.outWidth, bounds.outHeight) / (sample * 2) >= MAX_SIDE) sample *= 2
        val decoded = context.contentResolver.openInputStream(uri)?.use {
            BitmapFactory.decodeStream(it, null, BitmapFactory.Options().apply { inSampleSize = sample })
        } ?: throw IOException("can't decode $uri")
        val (w, h) = targetSize(decoded.width, decoded.height)
        if (w == decoded.width && h == decoded.height) return decoded
        return Bitmap.createScaledBitmap(decoded, w, h, true).also { decoded.recycle() }
    }
}
