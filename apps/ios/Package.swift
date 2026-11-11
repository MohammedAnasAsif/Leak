// swift-tools-version: 6.0
// Leak for iOS. Built and sideloaded with xtool (https://xtool.sh), no Mac or paid account needed.
import PackageDescription

let package = Package(
    name: "Leak",
    platforms: [.iOS(.v17)],
    products: [
        .library(name: "Leak", targets: ["Leak"])
    ],
    targets: [
        .target(name: "Leak")
    ]
)
