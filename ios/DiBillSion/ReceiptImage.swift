import UIKit

/// Receipt photos are shrunk on the phone before upload (PLAN §3.5): the long side to
/// `maxSide` px, JPEG at 0.85. That keeps the text readable for the model and the upload
/// around 200–400 KB.
enum ReceiptImage {
    static let maxSide = 1600

    /// The size to scale `width × height` to, never upscaling.
    static func targetSize(width: Int, height: Int, maxSide: Int = maxSide) -> (Int, Int) {
        let longest = max(width, height)
        guard longest > maxSide else { return (width, height) }
        let w = max(1, (width * maxSide + longest / 2) / longest)
        let h = max(1, (height * maxSide + longest / 2) / longest)
        return (w, h)
    }

    /// JPEG bytes ready for `scanReceipt`, upright (the renderer applies the orientation).
    static func jpeg(_ image: UIImage) -> Data? {
        let pixels = (Int(image.size.width * image.scale), Int(image.size.height * image.scale))
        let (w, h) = targetSize(width: pixels.0, height: pixels.1)
        let format = UIGraphicsImageRendererFormat()
        format.scale = 1
        format.opaque = true
        let size = CGSize(width: w, height: h)
        let resized = UIGraphicsImageRenderer(size: size, format: format).image { _ in
            image.draw(in: CGRect(origin: .zero, size: size))
        }
        return resized.jpegData(compressionQuality: 0.85)
    }
}
