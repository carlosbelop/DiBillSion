import SwiftUI

@main
struct DiBillSionApp: App {
    @State private var model: AppModel

    init() {
        // Unit tests run inside the app; give them an in-memory core instead of the real database.
        if ProcessInfo.processInfo.environment["XCTestConfigurationFilePath"] != nil,
           let core = try? Core.inMemory() {
            _model = State(initialValue: AppModel(core: core, transport: nil))
        } else {
            _model = State(initialValue: AppModel.live())
        }
    }

    var body: some Scene {
        WindowGroup {
            GroupsView()
                .environment(model)
                // `dibillsion://join/<code>` opens the app and joins the group.
                .onOpenURL { url in
                    guard url.scheme == "dibillsion", url.host == "join" else { return }
                    Task { await model.join(url.lastPathComponent) }
                }
        }
    }
}
