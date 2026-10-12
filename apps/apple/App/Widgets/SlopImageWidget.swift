import AppIntents
import AppKit
import HitSlopWidgets
import SwiftUI
import WidgetKit

struct HitSlopExtensionIntents: AppIntentsPackage {
  static var includedPackages: [any AppIntentsPackage.Type] { [HitSlopWidgetIntents.self] }
}

private struct Entry: TimelineEntry {
  let date: Date
  let id: String?
  let title: String
  let image: NSImage?
}

private struct Provider: AppIntentTimelineProvider {
  func placeholder(in context: Context) -> Entry { empty() }
  func snapshot(for configuration: SlopWidgetIntent, in context: Context) async -> Entry {
    entry(configuration, context: context)
  }
  func timeline(for configuration: SlopWidgetIntent, in context: Context) async -> Timeline<Entry> {
    Timeline(entries: [entry(configuration, context: context)], policy: .never)
  }
  private func empty() -> Entry { Entry(date: Date(), id: nil, title: "Choose a Slop", image: nil) }
  private func entry(_ configuration: SlopWidgetIntent, context: Context) -> Entry {
    guard let selection = configuration.slop else { return empty() }
    guard let cache = try? WidgetCache.shared(), let record = cache.record(id: selection.id) else {
      return Entry(date: Date(), id: selection.id, title: selection.title, image: nil)
    }
    let data = cache.imageData(
      record, context.family == .systemSmall ? .small : context.family == .systemMedium ? .medium : .large)
    return Entry(date: Date(), id: record.id, title: record.title, image: data.flatMap { NSImage(data: $0) })
  }
}

private struct ImageView: View {
  let entry: Entry
  var body: some View {
    Group {
      if let image = entry.image {
        Image(nsImage: image).renderingMode(.original).resizable()
          .widgetAccentedRenderingMode(.fullColor).scaledToFit()
          .accessibilityLabel("Open \(entry.title)")
      } else {
        VStack(spacing: 8) {
          Text(entry.title).font(.headline)
          Text(
            entry.id == nil
              ? "Edit Widget to choose a document. Open and close a slop with a widget to add it here."
              : "Open this slop in hitSlop to refresh its widget."
          )
          .font(.caption).multilineTextAlignment(.center)
        }.padding()
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .containerBackground(for: .widget) { Color.clear }
    .widgetURL(entry.id.map { WidgetCache.openURL(id: $0) } ?? URL(string: "hitslop://widgets"))
  }
}

@main struct SlopImageWidget: Widget {
  var body: some WidgetConfiguration {
    AppIntentConfiguration(kind: WidgetCache.kind, intent: SlopWidgetIntent.self, provider: Provider()) {
      ImageView(entry: $0)
    }
    .configurationDisplayName("Slop")
    .description("A piece of your slop, just as you made it.")
    .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    .contentMarginsDisabled()
    .containerBackgroundRemovable(false)
  }
}
