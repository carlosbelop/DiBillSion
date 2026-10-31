import SwiftUI
import UIKit

extension Color {
    static let teal = Color(red: 0x1F / 255, green: 0x6F / 255, blue: 0x5C / 255)
    static let amber = Color(red: 0xF2 / 255, green: 0xC1 / 255, blue: 0x4E / 255)
    static let coral = Color(red: 0xE0 / 255, green: 0x6C / 255, blue: 0x55 / 255)
    static let owed = Color(red: 0x2E / 255, green: 0x9E / 255, blue: 0x6B / 255)

    /// A stable colour per person or group, from its id.
    static func avatar(_ id: String) -> Color {
        let palette: [Color] = [.teal, .coral, .indigo, .orange, .pink, .mint, .purple, .brown]
        let hash = id.unicodeScalars.reduce(UInt32(7)) { ($0 &* 31) &+ $1.value }
        return palette[Int(hash % UInt32(palette.count))]
    }
}

/// A circle with an emoji or the first letter of a name.
struct Avatar: View {
    let id: String
    let name: String
    var emoji: String?
    var size: CGFloat = 40

    var body: some View {
        ZStack {
            Circle().fill(Color.avatar(id).opacity(emoji == nil ? 1 : 0.18))
            if let emoji, !emoji.isEmpty {
                Text(emoji).font(.system(size: size * 0.5))
            } else {
                Text(name.first.map { String($0).uppercased() } ?? "?")
                    .font(.system(size: size * 0.42, weight: .semibold))
                    .foregroundStyle(.white)
            }
        }
        .frame(width: size, height: size)
        .accessibilityHidden(true)
    }
}

/// "You owe 12,00 EUR" / "You are owed …" / "You are all square", coloured.
struct MyBalanceLabel: View {
    let minor: Int64
    let format: (Int64) -> String

    var body: some View {
        if minor < 0 {
            Text("You owe \(format(-minor))").foregroundStyle(Color.coral)
        } else if minor > 0 {
            Text("You are owed \(format(minor))").foregroundStyle(Color.owed)
        } else {
            Text("You are all square").foregroundStyle(.secondary)
        }
    }
}

extension GroupKindDto: CaseIterable, Identifiable {
    public static var allCases: [GroupKindDto] { [.home, .trip, .couple, .event, .project, .other] }
    public var id: Self { self }

    var label: LocalizedStringKey {
        switch self {
        case .trip: "Trip"
        case .home: "Home"
        case .couple: "Couple"
        case .event: "Event"
        case .project: "Project"
        case .other: "Other"
        }
    }

    var symbol: String {
        switch self {
        case .trip: "airplane"
        case .home: "house"
        case .couple: "heart"
        case .event: "party.popper"
        case .project: "hammer"
        case .other: "square.grid.2x2"
        }
    }
}

/// UIKit's share sheet, for invite links.
struct ShareSheet: UIViewControllerRepresentable {
    let items: [Any]

    func makeUIViewController(context: Context) -> UIActivityViewController {
        UIActivityViewController(activityItems: items, applicationActivities: nil)
    }

    func updateUIViewController(_ controller: UIActivityViewController, context: Context) {}
}
