import AppIntents
import AppKit
import HitSlopWidgets
import SwiftUI
import WidgetKit

private struct Entry: TimelineEntry {
  let date: Date
  let id: String?
  let title: String
  let image: NSImage?
  var generation: String? = nil
}

private struct Provider: AppIntentTimelineProvider {
  func placeholder(in context: Context) -> Entry { empty() }
  func snapshot(for configuration: SlopWidgetIntent, in context: Context) async -> Entry {
    entry(configuration, context: context)
  }
  func timeline(for configuration: SlopWidgetIntent, in context: Context) async -> Timeline<Entry> {
    Timeline(entries: [entry(configuration, context: context)], policy: .never)
  }
  private func empty() -> Entry { Entry(date: Date(), id: nil, title: "Choose a document", image: nil) }
  private func entry(_ configuration: SlopWidgetIntent, context: Context) -> Entry {
    guard let selection = configuration.slop else { return empty() }
    guard let cache = try? WidgetCache.shared(), let record = cache.record(id: selection.id) else {
      return Entry(date: Date(), id: selection.id, title: selection.title, image: nil)
    }
    let data = cache.imageData(
      record, context.family == .systemSmall ? .small : context.family == .systemMedium ? .medium : .large)
    return Entry(
      date: Date(), id: record.id, title: record.title, image: data.flatMap { NSImage(data: $0) },
      generation: record.generation)
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
          .id(entry.generation)
      } else {
        VStack(spacing: 8) {
          Text(entry.title).font(.headline)
          Text(
            entry.id == nil
              ? "Right-click, then choose Edit Widget → Document."
              : "Open this document in hitSlop to refresh its widget."
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
    .configurationDisplayName("Document")
    .description("Keep a slop on your desktop. Click to open it.")
    .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    .contentMarginsDisabled()
    .containerBackgroundRemovable(false)
  }
}
