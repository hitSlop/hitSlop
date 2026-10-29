import AppIntents
import SwiftUI
import TinysWidgets
import WidgetKit

#if os(macOS)
  import AppKit
  private typealias PlatformImage = NSImage
#else
  import UIKit
  private typealias PlatformImage = UIImage
#endif

private struct Entry: TimelineEntry {
  let date: Date
  let id: String?
  let title: String
  let image: PlatformImage?
  var generation: String? = nil
}

private struct Provider: AppIntentTimelineProvider {
  func placeholder(in context: Context) -> Entry { empty() }
  func snapshot(for configuration: TinyWidgetIntent, in context: Context) async -> Entry {
    entry(configuration, context: context)
  }
  func timeline(for configuration: TinyWidgetIntent, in context: Context) async -> Timeline<Entry> {
    Timeline(entries: [entry(configuration, context: context)], policy: .never)
  }
  private func empty() -> Entry { Entry(date: Date(), id: nil, title: "Choose a document", image: nil) }
  private func entry(_ configuration: TinyWidgetIntent, context: Context) -> Entry {
    guard let selection = configuration.tiny else { return empty() }
    guard let cache = try? WidgetCache.shared(), let record = cache.record(id: selection.id) else {
      return Entry(date: Date(), id: selection.id, title: selection.title, image: nil)
    }
    #if os(iOS)
      let title = cache.libraryIndex().first(where: { $0.id == record.id })?.title ?? record.title
    #else
      let title = record.title
    #endif
    let data = cache.imageData(
      record, context.family == .systemSmall ? .small : context.family == .systemMedium ? .medium : .large)
    return Entry(
      date: Date(), id: record.id, title: title, image: data.flatMap { PlatformImage(data: $0) },
      generation: record.generation)
  }
}

private struct ImageView: View {
  let entry: Entry
  var body: some View {
    Group {
      if let image = entry.image {
        platformImage(image).renderingMode(.original).resizable()
          .widgetAccentedRenderingMode(.fullColor).scaledToFit()
          .accessibilityLabel("Open \(entry.title)")
          .id(entry.generation)
      } else {
        VStack(spacing: 8) {
          Text(entry.title).font(.headline)
          #if os(macOS)
            Text(
              entry.id == nil
                ? "Right-click, then choose Edit Widget → Document."
                : "Open this document in tinys to refresh its widget."
            )
            .font(.caption).multilineTextAlignment(.center)
          #else
            Text(
              entry.id == nil
                ? "Touch and hold, then choose Edit Widget."
                : "Open tinys to update this widget."
            )
            .font(.caption).multilineTextAlignment(.center)
          #endif
        }.padding()
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity)
    .containerBackground(for: .widget) { Color.clear }
    .widgetURL(entry.id.map { WidgetCache.openURL(id: $0) } ?? URL(string: "tinys://widgets"))
  }
  private func platformImage(_ image: PlatformImage) -> Image {
    #if os(macOS)
      Image(nsImage: image)
    #else
      Image(uiImage: image)
    #endif
  }
}

@main struct TinyImageWidget: Widget {
  var body: some WidgetConfiguration {
    AppIntentConfiguration(kind: WidgetCache.kind, intent: TinyWidgetIntent.self, provider: Provider()) {
      ImageView(entry: $0)
    }
    .configurationDisplayName("Document")
    #if os(macOS)
      .description("Keep a tiny on your desktop. Click to open it.")
    #else
      .description("Keep a tiny on your Home Screen. Tap to open it.")
    #endif
    .supportedFamilies([.systemSmall, .systemMedium, .systemLarge])
    .contentMarginsDisabled()
    .containerBackgroundRemovable(false)
  }
}
