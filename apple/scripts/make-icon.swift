// Draws the app icon: three connected nodes on a gradient. No outside artwork is used.
//
//   swift apple/scripts/make-icon.swift apple/App/Assets.xcassets/AppIcon.appiconset/AppIcon.png
//
// The App Store rejects icons with an alpha channel, so the bitmap has none.
import CoreGraphics
import Foundation
import ImageIO
import UniformTypeIdentifiers

let size = 1024
guard CommandLine.arguments.count == 2 else {
    FileHandle.standardError.write(Data("usage: make-icon.swift <output.png>\n".utf8))
    exit(2)
}
let outputURL = URL(fileURLWithPath: CommandLine.arguments[1])

let colorSpace = CGColorSpace(name: CGColorSpace.sRGB)!
guard let context = CGContext(
    data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
    space: colorSpace, bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue
) else { fatalError("cannot create a bitmap") }

let canvas = CGFloat(size)
let gradient = CGGradient(
    colorsSpace: colorSpace,
    colors: [
        CGColor(srgbRed: 0.30, green: 0.27, blue: 0.80, alpha: 1),
        CGColor(srgbRed: 0.10, green: 0.45, blue: 0.95, alpha: 1),
    ] as CFArray,
    locations: [0, 1]
)!
context.drawLinearGradient(
    gradient, start: CGPoint(x: 0, y: canvas), end: CGPoint(x: canvas, y: 0), options: []
)

let top = CGPoint(x: 512, y: 700)
let left = CGPoint(x: 290, y: 330)
let right = CGPoint(x: 734, y: 330)

context.setStrokeColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 0.65))
context.setLineWidth(34)
context.setLineCap(.round)
context.move(to: left)
context.addLine(to: top)
context.addLine(to: right)
context.addLine(to: left)
context.strokePath()

context.setFillColor(CGColor(srgbRed: 1, green: 1, blue: 1, alpha: 1))
for (center, radius) in [(top, CGFloat(110)), (left, 82), (right, 82)] {
    context.fillEllipse(in: CGRect(
        x: center.x - radius, y: center.y - radius, width: radius * 2, height: radius * 2
    ))
}

guard let image = context.makeImage(),
      let destination = CGImageDestinationCreateWithURL(
        outputURL as CFURL, UTType.png.identifier as CFString, 1, nil
      )
else { fatalError("cannot write the icon") }
CGImageDestinationAddImage(destination, image, nil)
guard CGImageDestinationFinalize(destination) else { fatalError("cannot write the icon") }
