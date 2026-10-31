import PhotosUI
import SwiftUI
import UIKit

/// Scan a receipt: a photo now, or an image already on the phone.
struct ScanReceiptButton: View {
    @Environment(AppModel.self) private var model
    @State private var picking = false
    @State private var shooting = false
    @State private var item: PhotosPickerItem?

    var body: some View {
        Menu {
            if UIImagePickerController.isSourceTypeAvailable(.camera) {
                Button { shooting = true } label: { Label("Take a photo", systemImage: "camera") }
            }
            Button { picking = true } label: { Label("Choose an image", systemImage: "photo") }
        } label: {
            Label("Scan receipt", systemImage: "doc.text.viewfinder")
        }
        .photosPicker(isPresented: $picking, selection: $item, matching: .images)
        .onChange(of: item) { _, picked in
            guard let picked else { return }
            item = nil
            Task {
                let data = try? await picked.loadTransferable(type: Data.self)
                send(data.flatMap(UIImage.init(data:)))
            }
        }
        .fullScreenCover(isPresented: $shooting) {
            CameraPicker { image in
                shooting = false
                if let image { send(image) }
            }
            .ignoresSafeArea()
        }
    }

    private func send(_ image: UIImage?) {
        if let jpeg = image.flatMap(ReceiptImage.jpeg) {
            model.scanReceipt(jpeg: jpeg)
        } else {
            model.receiptImageFailed()
        }
    }
}

/// The system camera; no photo library access needed.
private struct CameraPicker: UIViewControllerRepresentable {
    let done: (UIImage?) -> Void

    func makeUIViewController(context: Context) -> UIImagePickerController {
        let picker = UIImagePickerController()
        picker.sourceType = .camera
        picker.delegate = context.coordinator
        return picker
    }

    func updateUIViewController(_ controller: UIImagePickerController, context: Context) {}

    func makeCoordinator() -> Coordinator { Coordinator(done: done) }

    final class Coordinator: NSObject, UIImagePickerControllerDelegate, UINavigationControllerDelegate {
        let done: (UIImage?) -> Void
        init(done: @escaping (UIImage?) -> Void) { self.done = done }

        func imagePickerController(_ picker: UIImagePickerController,
                                   didFinishPickingMediaWithInfo info: [UIImagePickerController.InfoKey: Any]) {
            done(info[.originalImage] as? UIImage)
        }

        func imagePickerControllerDidCancel(_ picker: UIImagePickerController) { done(nil) }
    }
}

/// The scan in progress, over the group: reading, then the review.
struct ReceiptFlowView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        switch model.scan {
        case .review(let draft):
            if let d = model.detail { ReceiptReviewView(d: d, draft: draft) }
        default:
            ReadingReceiptView()
        }
    }
}

/// Shown while the server's model reads the photo.
private struct ReadingReceiptView: View {
    @Environment(AppModel.self) private var model

    var body: some View {
        NavigationStack {
            VStack(spacing: 16) {
                ProgressView().controlSize(.large)
                Text("Reading the receipt…").font(.headline)
                Text("This takes a few seconds. You can check and fix everything afterwards.")
                    .font(.subheadline).foregroundStyle(.secondary).multilineTextAlignment(.center)
            }
            .padding(32)
            .navigationTitle("Scan receipt")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { model.closeScan() } }
            }
        }
    }
}

/// Review of a scanned receipt (SPEC flow 8): fix what the model misread, choose who shares
/// each item (everyone by default) and save it as one expense with an item per line.
private struct ReceiptReviewView: View {
    let d: GroupDetail
    let draft: ReceiptDraft
    @Environment(AppModel.self) private var model
    @State private var confirmDiscard = false
    @State private var saving = false

    private var active: [MemberDto] { d.members.filter(\.active) }
    private var everyone: Set<String> { Set(active.map(\.id)) }
    private var exponent: Int? { model.exponent(of: draft.currency) }
    private var foreign: Bool { draft.currency != d.group.baseCurrency }
    private var rateValue: String? { foreign ? Money.parseRate(draft.rate) : nil }
    private var problem: ReceiptDraft.Problem? { exponent.flatMap { draft.problem(exponent: $0) } }
    private var canSave: Bool {
        exponent != nil && problem == nil && !draft.title.trimmingCharacters(in: .whitespaces).isEmpty
            && !draft.payerId.isEmpty && (!foreign || rateValue != nil) && !saving
    }

    var body: some View {
        NavigationStack {
            Form {
                Section {
                    TextField("Description", text: binding(\.title))
                    Picker("Paid by", selection: binding(\.payerId)) {
                        ForEach(active, id: \.id) { m in Text(m.displayName).tag(m.id) }
                    }
                    DatePicker("Date", selection: Binding(
                        get: { DateDays.date(draft.dateDays) },
                        set: { v in model.editReceipt { $0.dateDays = DateDays.from(v) } }),
                               displayedComponents: .date)
                    HStack {
                        Text("Currency")
                        Spacer()
                        TextField("EUR", text: Binding(
                            get: { draft.currency },
                            set: { v in model.editReceipt { $0.currency = String(v.uppercased().filter(\.isLetter).prefix(3)) } }))
                            .textInputAutocapitalization(.characters)
                            .autocorrectionDisabled()
                            .multilineTextAlignment(.trailing)
                            .frame(width: 70)
                    }
                    if foreign {
                        TextField("Rate: 1 \(draft.currency) = ? \(d.group.baseCurrency)", text: binding(\.rate))
                            .keyboardType(.decimalPad)
                    }
                } footer: {
                    if let exponent { TotalCheck(draft: draft, exponent: exponent) }
                }

                ForEach(draft.lines) { line in
                    Section { lineEditor(line) }
                }
                Section {
                    Button { model.editReceipt { $0.add(everyone: everyone) } } label: {
                        Label("Add item", systemImage: "plus")
                    }
                } footer: {
                    Text("Tap people to choose who shares each item. Everyone shares it until you do.")
                }

                if let exponent {
                    let shares = model.receiptShares(draft, memberOrder: d.members.map(\.id))
                    if !shares.isEmpty {
                        Section("Per person") {
                            ForEach(active.filter { shares[$0.id] != nil }, id: \.id) { m in
                                HStack {
                                    Avatar(id: m.id, name: m.displayName, size: 28)
                                    Text(m.displayName)
                                    Spacer()
                                    Text(Money.format(shares[m.id] ?? 0, exponent: exponent, code: draft.currency))
                                        .monospacedDigit().fontWeight(.semibold)
                                }
                            }
                        }
                    }
                }
                Section {
                    EmptyView()
                } footer: {
                    VStack(alignment: .leading, spacing: 8) {
                        if let problem { Text(message(problem)).foregroundStyle(Color.coral) }
                        Text("\(draft.scansLeft) scans left today")
                    }
                }
            }
            .navigationTitle("Check the receipt")
            .navigationBarTitleDisplayMode(.inline)
            .interactiveDismissDisabled()
            .toolbar {
                ToolbarItem(placement: .cancellationAction) { Button("Cancel") { confirmDiscard = true } }
                ToolbarItem(placement: .confirmationAction) {
                    Button("Save") {
                        saving = true
                        Task {
                            await model.saveReceipt()
                            saving = false
                        }
                    }
                    .disabled(!canSave)
                }
            }
            .alert("DiBillSion", isPresented: Binding(get: { model.message != nil },
                                                      set: { if !$0 { model.message = nil } })) {
                Button("OK", role: .cancel) {}
            } message: {
                Text(model.message ?? "")
            }
            .confirmationDialog("Discard this receipt?", isPresented: $confirmDiscard, titleVisibility: .visible) {
                Button("Discard", role: .destructive) { model.closeScan() }
            }
        }
    }

    @ViewBuilder
    private func lineEditor(_ line: ReceiptLine) -> some View {
        HStack {
            TextField("Item", text: lineBinding(line.id, \.label))
            TextField("0", text: lineBinding(line.id, \.amount))
                .keyboardType(.decimalPad)
                .multilineTextAlignment(.trailing)
                .monospacedDigit()
                .frame(width: 96)
                .foregroundStyle(exponent.map { Money.parse(line.amount, exponent: $0) == nil } == true ? Color.coral : .primary)
            Button(role: .destructive) { model.editReceipt { $0.remove(line.id) } } label: {
                Image(systemName: "trash")
            }
            .buttonStyle(.borderless)
            .accessibilityLabel("Delete item")
        }
        if line.discountMinor != 0, let exponent {
            Text("Includes a \(Money.format(-line.discountMinor, exponent: exponent, code: draft.currency)) discount")
                .font(.caption).foregroundStyle(.secondary)
        }
        ScrollView(.horizontal, showsIndicators: false) {
            HStack(spacing: 8) {
                let all = line.members.isSuperset(of: everyone)
                chip(Text("Everyone"), selected: all) {
                    model.editReceipt { $0.update(line.id) { $0.members = all ? [] : everyone } }
                }
                ForEach(active, id: \.id) { m in
                    let on = line.members.contains(m.id)
                    chip(Text(m.displayName), selected: on && !all) {
                        // From "everyone", a tap picks just that person: the usual case.
                        model.editReceipt {
                            $0.update(line.id) { l in
                                if all { l.members = [m.id] } else if on { l.members.remove(m.id) } else { l.members.insert(m.id) }
                            }
                        }
                    }
                }
            }
        }
    }

    private func chip(_ label: Text, selected: Bool, action: @escaping () -> Void) -> some View {
        Button(action: action) {
            label.font(.subheadline)
                .padding(.horizontal, 12).padding(.vertical, 6)
                .background(Capsule().fill(selected ? Color.teal : Color(.secondarySystemFill)))
                .foregroundStyle(selected ? .white : .primary)
        }
        .buttonStyle(.plain)
    }

    private func message(_ problem: ReceiptDraft.Problem) -> String {
        switch problem {
        case .amount: String(localized: "Every item needs a valid amount (0 for free items).")
        case .nobody: String(localized: "Every item needs at least one person.")
        case .empty: String(localized: "Enter a valid positive amount.")
        }
    }

    private func binding(_ key: WritableKeyPath<ReceiptDraft, String>) -> Binding<String> {
        Binding(get: { draft[keyPath: key] }, set: { v in model.editReceipt { $0[keyPath: key] = v } })
    }

    private func lineBinding(_ id: Int, _ key: WritableKeyPath<ReceiptLine, String>) -> Binding<String> {
        Binding(
            get: { draft.lines.first { $0.id == id }?[keyPath: key] ?? "" },
            set: { v in model.editReceipt { $0.update(id) { $0[keyPath: key] = v } } })
    }
}

/// Whether the items add up to the printed total: the one check worth reading.
private struct TotalCheck: View {
    let draft: ReceiptDraft
    let exponent: Int

    var body: some View {
        if let sum = draft.sum(exponent: exponent) {
            let fmt = { (m: Int64) in Money.format(m, exponent: exponent, code: draft.currency) }
            Group {
                if let printed = draft.printedTotalMinor {
                    if printed == sum {
                        Label("Items add up to the receipt total, \(fmt(sum)).", systemImage: "checkmark.circle.fill")
                            .foregroundStyle(Color.owed)
                    } else {
                        Label("Items add up to \(fmt(sum)) but the receipt says \(fmt(printed)). Check the amounts.",
                              systemImage: "exclamationmark.triangle.fill")
                            .foregroundStyle(Color.coral)
                    }
                } else {
                    Label("No total found on the receipt. Items add up to \(fmt(sum)).",
                          systemImage: "exclamationmark.triangle.fill")
                        .foregroundStyle(Color.coral)
                }
            }
            .font(.subheadline)
            .padding(.top, 4)
        }
    }
}
