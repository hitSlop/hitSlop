import AppKit
import HitSlopCore
import HitSlopFeatures
import SwiftUI

struct CatalogSidebar: View {
  let filter: CatalogFilter
  let categories: [SlopCategory]
  let recentCount: Int
  let select: (CatalogFilter) -> Void

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      HStack(spacing: 10) {
        Image(nsImage: NSApplication.shared.applicationIconImage)
          .resizable().interpolation(.high).scaledToFit().frame(width: 30, height: 30)
        Text("hitSlop").font(.title3.weight(.bold))
      }
      .padding(.horizontal, 16).padding(.top, 6).padding(.bottom, 18)

      ScrollView {
        VStack(alignment: .leading, spacing: 2) {
          SidebarSection("Library")
          SidebarButton(title: "Recents", icon: .symbol("clock"), count: recentCount, selected: filter == .recents) {
            select(.recents)
          }
          SidebarButton(title: "Templates", icon: .symbol("square.grid.2x2"), selected: filter == .all) { select(.all) }

          if !categories.isEmpty {
            SidebarSection("Categories").padding(.top, 14)
            ForEach(categories, id: \.self) { category in
              SidebarButton(
                title: category.label, icon: .emoji(category.emoji), selected: filter == .category(category)
              ) { select(.category(category)) }
            }
          }
        }
        .padding(.horizontal, 10).padding(.bottom, 18)
      }

      AuthoringLink()
        .padding(.horizontal, 12).padding(.bottom, 12)

      HStack(spacing: 2) {
        BrandLink(name: "github", label: "GitHub", destination: CatalogLinks.github)
        BrandLink(name: "discord", label: "Discord", destination: CatalogLinks.discord)
      }
      .padding(.horizontal, 12).padding(.vertical, 10)
      .frame(maxWidth: .infinity, alignment: .leading)
      .overlay(alignment: .top) { Rectangle().fill(CatalogStyle.hairline).frame(height: 1).padding(.horizontal, 14) }
    }
    .background(CatalogStyle.sidebar)
  }
}

private struct AuthoringLink: View {
  @State private var isHovering = false

  var body: some View {
    Link(destination: CatalogLinks.authoring) {
      HStack(alignment: .top, spacing: 8) {
        Text("Learn how to make your own slop")
          .font(.callout.weight(.medium))
          .multilineTextAlignment(.leading)
          .fixedSize(horizontal: false, vertical: true)
          .frame(maxWidth: .infinity, alignment: .leading)
        Image(systemName: "arrow.up.right")
          .font(.caption.weight(.semibold))
          .padding(.top, 2)
          .accessibilityHidden(true)
      }
      .foregroundStyle(isHovering ? CatalogStyle.brandHover : CatalogStyle.brand)
      .padding(12)
      .background(
        CatalogStyle.brand.opacity(isHovering ? 0.14 : 0.08),
        in: RoundedRectangle(cornerRadius: 10, style: .continuous)
      )
      .contentShape(RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
    .buttonStyle(.plain)
    .onHover { isHovering = $0 }
    .help("Open the guide in your browser")
    .accessibilityLabel("Learn how to make your own slop")
    .accessibilityHint("Opens the guide in your browser")
  }
}

private struct SidebarSection: View {
  let title: String
  init(_ title: String) { self.title = title }
  var body: some View {
    Text(title)
      .font(.caption.weight(.medium)).foregroundStyle(.secondary)
      .padding(.horizontal, 10).padding(.bottom, 4)
  }
}

private struct BrandLink: View {
  let name: String
  let label: String
  let destination: URL?
  @State private var isHovering = false

  var body: some View {
    Group {
      if let destination {
        Link(destination: destination) { icon }
      } else {
        Button {
        } label: {
          icon
        }
        .disabled(true)
      }
    }
    .buttonStyle(.plain)
    .onHover { isHovering = $0 }
    .help(destination == nil ? "\(label) — coming soon" : label)
    .accessibilityLabel(destination == nil ? "\(label) — coming soon" : label)
  }

  private var icon: some View {
    Group {
      if let image = brandImage(named: name) {
        Image(nsImage: image).resizable().scaledToFit()
      } else {
        Image(systemName: "link")
      }
    }
    .frame(width: 14, height: 14)
    .frame(width: 28, height: 28)
    .foregroundStyle(isHovering ? .primary : .secondary)
    .background(isHovering ? CatalogStyle.hover : .clear, in: RoundedRectangle(cornerRadius: 7, style: .continuous))
    .contentShape(Rectangle())
  }
}

private enum SidebarIcon {
  case symbol(String)
  case emoji(String)
}

private struct SidebarButton: View {
  let title: String
  let icon: SidebarIcon
  var count: Int?
  let selected: Bool
  let action: () -> Void
  @State private var isHovering = false

  init(title: String, icon: SidebarIcon, count: Int? = nil, selected: Bool, action: @escaping () -> Void) {
    self.title = title
    self.icon = icon
    self.count = count
    self.selected = selected
    self.action = action
  }

  var body: some View {
    Button(action: action) {
      HStack(spacing: 9) {
        Group {
          switch icon {
          case .symbol(let name):
            Image(systemName: name).font(.system(size: 14, weight: .medium))
              .foregroundStyle(selected ? CatalogStyle.brand : .secondary)
          case .emoji(let emoji):
            Text(emoji).font(.system(size: 14))
          }
        }
        .frame(width: 20).accessibilityHidden(true)
        Text(title).lineLimit(1)
        Spacer(minLength: 4)
        if let count, count > 0 {
          Text("\(count)").font(.caption.monospacedDigit()).foregroundStyle(.tertiary)
        }
      }
      .font(.body.weight(selected ? .medium : .regular))
      .padding(.horizontal, 10).frame(height: 32)
      .background(
        selected ? CatalogStyle.selection : isHovering ? CatalogStyle.hover : .clear,
        in: RoundedRectangle(cornerRadius: 8, style: .continuous)
      )
      .contentShape(Rectangle())
    }
    .buttonStyle(.plain)
    .onHover { isHovering = $0 }
  }
}

private func brandImage(named name: String) -> NSImage? {
  guard let url = Bundle.module.url(forResource: name, withExtension: "svg"), let image = NSImage(contentsOf: url)
  else { return nil }
  image.isTemplate = true
  return image
}
