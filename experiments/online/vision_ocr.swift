// Local evaluation helper. Screen text and display time are not acoustic truth.
import CoreGraphics
import Foundation
import ImageIO
import Vision

func emit(_ value: [String: Any], to file: FileHandle) throws {
    var data = try JSONSerialization.data(withJSONObject: value, options: [.sortedKeys])
    data.append(10)
    try file.write(contentsOf: data)
}

func pixels(_ image: CGImage, width: Int, height: Int) -> [UInt8] {
    var bytes = [UInt8](repeating: 0, count: width * height * 4)
    bytes.withUnsafeMutableBytes { buffer in
        let context = CGContext(data: buffer.baseAddress, width: width, height: height,
                                bitsPerComponent: 8, bytesPerRow: width * 4,
                                space: CGColorSpaceCreateDeviceRGB(),
                                bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue
                                    | CGBitmapInfo.byteOrder32Big.rawValue)!
        context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    }
    return bytes
}

// Mean of darkest 5% of pixels: diagnostic for black current vs faded context.
// It is not a language, correctness, or speaker classifier.
func inkLuma(_ image: CGImage, box: CGRect) -> Double? {
    let rect = CGRect(x: box.minX * Double(image.width),
                      y: (1 - box.maxY) * Double(image.height),
                      width: box.width * Double(image.width),
                      height: box.height * Double(image.height)).integral
    guard let crop = image.cropping(to: rect), crop.width > 0, crop.height > 0 else { return nil }
    let bytes = pixels(crop, width: crop.width, height: crop.height)
    var bins = [Int](repeating: 0, count: 256)
    for i in stride(from: 0, to: bytes.count, by: 4) {
        let luma = (54 * Int(bytes[i]) + 183 * Int(bytes[i + 1]) + 19 * Int(bytes[i + 2])) / 256
        bins[luma] += 1
    }
    var remaining = max(1, crop.width * crop.height / 20)
    let count = remaining
    var sum = 0
    for i in bins.indices {
        let take = min(remaining, bins[i])
        sum += take * i
        remaining -= take
        if remaining == 0 { break }
    }
    return Double(sum) / Double(count)
}

guard CommandLine.arguments.count == 3 else {
    fputs("usage: vision-ocr jobs.json output.jsonl\n", stderr)
    exit(2)
}
let jobsURL = URL(fileURLWithPath: CommandLine.arguments[1])
let outputURL = URL(fileURLWithPath: CommandLine.arguments[2])
guard !FileManager.default.fileExists(atPath: outputURL.path) else {
    fputs("refusing to overwrite OCR evidence\n", stderr)
    exit(2)
}
let jobs = try JSONSerialization.jsonObject(with: Data(contentsOf: jobsURL)) as! [[String: Any]]
FileManager.default.createFile(atPath: outputURL.path, contents: nil)
let file = try FileHandle(forWritingTo: outputURL)
var previousPixels: [UInt8] = []
var previousLines: [[String: Any]] = []
var previousCase = ""
var previousOCRTime = -Double.infinity
var previousOCRFrame = ""
var failures = 0
for (index, job) in jobs.enumerated() {
    try autoreleasepool {
        var row = job
        let started = Date()
        do {
            let path = job["image_path"] as! String
            guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
                  let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
                throw NSError(domain: "vision-ocr", code: 1,
                              userInfo: [NSLocalizedDescriptionKey: "cannot decode frame"])
            }
            let caseID = job["case_id"] as! String
            let time = job["media_time_s"] as! Double
            let thumb = pixels(image, width: 256, height: 144)
            var sum = 0
            var changed = 0
            if previousCase == caseID, previousPixels.count == thumb.count {
                for i in stride(from: 0, to: thumb.count, by: 4) {
                    let delta = abs(Int(thumb[i]) - Int(previousPixels[i]))
                        + abs(Int(thumb[i + 1]) - Int(previousPixels[i + 1]))
                        + abs(Int(thumb[i + 2]) - Int(previousPixels[i + 2]))
                    sum += delta
                    if delta >= 36 { changed += 1 }
                }
            }
            let meanDelta = Double(sum) / Double(256 * 144 * 3)
            let changedFraction = Double(changed) / Double(256 * 144)
            let reuse = previousCase == caseID && time - previousOCRTime < 10
                && meanDelta < 0.1 && changedFraction < 0.0001
            row["mean_pixel_delta"] = meanDelta
            row["changed_pixel_fraction"] = changedFraction
            if reuse {
                row["lines"] = previousLines
                row["reused_from_frame"] = previousOCRFrame
            } else {
                let request = VNRecognizeTextRequest()
                request.recognitionLevel = .accurate
                request.recognitionLanguages = job["languages"] as? [String]
                    ?? ["ja-JP", "zh-Hans", "en-US"]
                request.automaticallyDetectsLanguage = false
                request.usesLanguageCorrection = false
                request.minimumTextHeight = 0.009
                if let roi = job["roi"] as? [Double], roi.count == 4 {
                    request.regionOfInterest = CGRect(x: roi[0], y: 1 - roi[1] - roi[3],
                                                      width: roi[2], height: roi[3])
                }
                try VNImageRequestHandler(cgImage: image).perform([request])
                let lines: [[String: Any]] = (request.results ?? []).compactMap { observation in
                    guard let best = observation.topCandidates(1).first else { return nil }
                    let local = observation.boundingBox
                    let roi = request.regionOfInterest
                    // Vision boxes are relative to the recognition ROI. Convert
                    // before reading image pixels or publishing full-frame boxes.
                    let box = CGRect(x: roi.minX + local.minX * roi.width,
                                     y: roi.minY + local.minY * roi.height,
                                     width: local.width * roi.width,
                                     height: local.height * roi.height)
                    var line: [String: Any] = [
                        "text": best.string, "confidence": best.confidence,
                        "box": [box.minX, 1 - box.maxY, box.width, box.height],
                        "alternatives": observation.topCandidates(3).map {
                            ["text": $0.string, "confidence": $0.confidence] as [String: Any]
                        }
                    ]
                    if let luma = inkLuma(image, box: box) { line["ink_luma"] = luma }
                    return line
                }
                row["lines"] = lines
                previousLines = lines
                previousOCRTime = time
                previousOCRFrame = job["frame_id"] as! String
                previousPixels = thumb
            }
            previousCase = caseID
            row["status"] = "completed"
        } catch {
            failures += 1
            previousCase = ""
            row["status"] = "failed"
            row["error"] = error.localizedDescription
        }
        row["processing_s"] = Date().timeIntervalSince(started)
        try emit(row, to: file)
    }
    if (index + 1) % 100 == 0 { fputs("OCR frames: \(index + 1)/\(jobs.count)\n", stderr) }
}
try file.close()
if failures > 0 { exit(1) }
