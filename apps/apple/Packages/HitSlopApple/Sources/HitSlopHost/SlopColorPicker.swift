import AppKit
import SwiftUI

/// A palette color as the open picker holds it. The palette stores only sRGB bytes, so the
/// picker keeps its own hue and saturation through the colors that lose them (grays and
/// black): a drag through gray comes back where it started.
struct SlopPickerColor: Equatable {
  var hue: Double
  var saturation: Double
  var brightness: Double
  var alpha: Double = 1

  init(hue: Double, saturation: Double, brightness: Double, alpha: Double = 1) {
    self.hue = hue
    self.saturation = saturation
    self.brightness = brightness
    self.alpha = alpha
  }
  init?(hex: String) {
    guard let components = SlopThemeColor.color(hex)?.components, components.count == 4 else { return nil }
    let (red, green, blue) = (Double(components[0]), Double(components[1]), Double(components[2]))
    let high = max(red, green, blue)
    let spread = high - min(red, green, blue)
    var hue = 0.0
    if spread > 0 {
      switch high {
      case red: hue = (green - blue) / spread
      case green: hue = (blue - red) / spread + 2
      default: hue = (red - green) / spread + 4
      }
      hue = (hue / 6 + 1).truncatingRemainder(dividingBy: 1)
    }
    self.init(hue: hue, saturation: high > 0 ? spread / high : 0, brightness: high, alpha: Double(components[3]))
  }

  /// Opaque sRGB components.
  var rgb: [Double] {
    let sector = hue * 6
    let whole = sector.rounded(.down)
    let fraction = sector - whole
    let (value, saturation) = (brightness, saturation)
    let low = value * (1 - saturation)
    let falling = value * (1 - saturation * fraction)
    let rising = value * (1 - saturation * (1 - fraction))
    // A hue of 1 is red again.
    switch Int(whole) % 6 {
    case 0: return [value, rising, low]
    case 1: return [falling, value, low]
    case 2: return [low, value, rising]
    case 3: return [low, falling, value]
    case 4: return [rising, low, value]
    default: return [value, low, falling]
    }
  }
  /// The palette's spelling of this color.
  var hex: String {
    let components = (rgb + [alpha]).map { CGFloat($0) }
    guard let color = CGColor(colorSpace: SlopThemeColor.space, components: components),
      let hex = SlopThemeColor.hex(color)
    else { return "#000000" }
    return hex
  }
  var opaque: Color { Color(.sRGB, red: rgb[0], green: rgb[1], blue: rgb[2]) }

  /// The color to show once the palette holds `hex`. The picker's own color keeps every
  /// handle where it is; a gray keeps the hue, and black keeps the saturation as well.
  func adopting(_ hex: String) -> SlopPickerColor {
    guard hex != self.hex, var next = SlopPickerColor(hex: hex) else { return self }
    if next.saturation == 0 { next.hue = hue }
    if next.brightness == 0 { next.saturation = saturation }
    return next
  }
}

/// The theme panel's color picker, open below one color's row: saturation and brightness,
/// hue, opacity, an eyedropper and the palette's colors. Every change is a palette change,
/// and the owner makes a drag one undo step.
struct SlopColorPicker: View {
  let hex: String
  let swatches: [String]
  let set: (String) -> Void
  @State private var color: SlopPickerColor

  init(hex: String, swatches: [String], set: @escaping (String) -> Void) {
    self.hex = hex
    self.swatches = swatches
    self.set = set
    _color = State(initialValue: SlopPickerColor(hex: hex) ?? SlopPickerColor(hue: 0, saturation: 0, brightness: 0))
  }

  var body: some View {
    VStack(alignment: .leading, spacing: 10) {
      SlopSaturationBrightnessField(color: color) { saturation, brightness in
        change {
          $0.saturation = saturation
          $0.brightness = brightness
        }
      }
      .frame(height: 140)
      HStack(spacing: 10) {
        VStack(spacing: 8) {
          SlopPickerStrip(label: "Hue", value: color.hue, knob: Color(hue: color.hue, saturation: 1, brightness: 1)) {
            LinearGradient(
              colors: (0...6).map { Color(hue: Double($0) / 6, saturation: 1, brightness: 1) },
              startPoint: .leading, endPoint: .trailing)
          } change: { hue in
            change { $0.hue = hue }
          }
          SlopPickerStrip(label: "Opacity", value: color.alpha, knob: color.opaque) {
            ZStack {
              SlopCheckerboard()
              LinearGradient(
                colors: [color.opaque.opacity(0), color.opaque], startPoint: .leading, endPoint: .trailing)
            }
          } change: { alpha in
            change { $0.alpha = alpha }
          }
        }
        Button(action: sample) {
          Image(systemName: "eyedropper").frame(width: 24, height: 24).contentShape(Rectangle())
        }
        .buttonStyle(.plain).foregroundStyle(.secondary)
        .help("Pick a color from the screen").accessibilityLabel("Pick a color from the screen")
      }
      LazyVGrid(
        columns: [GridItem(.adaptive(minimum: 18, maximum: 18), spacing: 7)], alignment: .leading, spacing: 7
      ) {
        ForEach(swatches, id: \.self) { swatch in
          Button {
            set(swatch)
          } label: {
            SlopSwatch(hex: swatch, selected: swatch == hex).frame(width: 18, height: 18)
          }
          .buttonStyle(.plain).help(swatch).accessibilityLabel(swatch)
        }
      }
    }
    .onChange(of: hex) { _, next in color = color.adopting(next) }
  }

  private func change(_ update: (inout SlopPickerColor) -> Void) {
    update(&color)
    set(color.hex)
  }
  private func sample() {
    NSColorSampler().show { picked in
      guard let picked, let hex = SlopThemeColor.hex(picked.cgColor) else { return }
      set(hex)
    }
  }
}

/// A palette color over a checkerboard, so a translucent one reads as translucent.
struct SlopSwatch: View {
  let hex: String
  var selected = false

  var body: some View {
    let shape = RoundedRectangle(cornerRadius: 5, style: .continuous)
    ZStack {
      SlopCheckerboard()
      Color(cgColor: SlopThemeColor.color(hex) ?? CGColor(gray: 0, alpha: 1))
    }
    .clipShape(shape)
    .overlay(shape.strokeBorder(.primary.opacity(0.15)))
    .overlay {
      if selected {
        RoundedRectangle(cornerRadius: 7, style: .continuous).strokeBorder(Color.accentColor, lineWidth: 2)
          .padding(-3)
      }
    }
  }
}

private struct SlopCheckerboard: View {
  var body: some View {
    Canvas { context, size in
      let square: CGFloat = 4
      var dark = Path()
      for row in 0..<Int((size.height / square).rounded(.up)) {
        for column in 0..<Int((size.width / square).rounded(.up)) where (row + column).isMultiple(of: 2) {
          dark.addRect(CGRect(x: CGFloat(column) * square, y: CGFloat(row) * square, width: square, height: square))
        }
      }
      context.fill(Path(CGRect(origin: .zero, size: size)), with: .color(.white))
      context.fill(dark, with: .color(Color(white: 0.82)))
    }
  }
}

private struct SlopPickerKnob: View {
  static let size: CGFloat = 14
  let color: Color

  var body: some View {
    Circle().fill(color)
      .overlay(Circle().strokeBorder(.white, lineWidth: 2))
      .shadow(color: .black.opacity(0.35), radius: 1.5, y: 0.5)
      .frame(width: Self.size, height: Self.size)
  }
}

/// Saturation across, brightness down, at the color's hue.
private struct SlopSaturationBrightnessField: View {
  let color: SlopPickerColor
  let change: (_ saturation: Double, _ brightness: Double) -> Void

  var body: some View {
    let shape = RoundedRectangle(cornerRadius: 8, style: .continuous)
    GeometryReader { proxy in
      let size = proxy.size
      ZStack(alignment: .topLeading) {
        ZStack {
          Color(hue: color.hue, saturation: 1, brightness: 1)
          LinearGradient(colors: [.white, .white.opacity(0)], startPoint: .leading, endPoint: .trailing)
          LinearGradient(colors: [.black.opacity(0), .black], startPoint: .top, endPoint: .bottom)
        }
        .clipShape(shape)
        .overlay(shape.strokeBorder(.primary.opacity(0.12)))
        SlopPickerKnob(color: color.opaque)
          .position(x: color.saturation * size.width, y: (1 - color.brightness) * size.height)
      }
      .contentShape(Rectangle())
      .gesture(
        DragGesture(minimumDistance: 0).onChanged { drag in
          change(unit(drag.location.x / size.width), 1 - unit(drag.location.y / size.height))
        })
    }
    .accessibilityElement()
    .accessibilityLabel("Saturation and brightness")
    .accessibilityValue(
      "\(Int((color.saturation * 100).rounded()))% saturation, \(Int((color.brightness * 100).rounded()))% brightness")
  }
}

/// A track from 0 to 1 with a knob that stays inside it.
private struct SlopPickerStrip<Fill: View>: View {
  let label: String
  let value: Double
  let knob: Color
  @ViewBuilder let fill: Fill
  let change: (Double) -> Void

  var body: some View {
    GeometryReader { proxy in
      let travel = max(proxy.size.width - SlopPickerKnob.size, 1)
      ZStack(alignment: .leading) {
        fill
          .frame(height: 10)
          .clipShape(Capsule())
          .overlay(Capsule().strokeBorder(.primary.opacity(0.12)))
        SlopPickerKnob(color: knob).offset(x: value * travel)
      }
      .frame(maxHeight: .infinity)
      .contentShape(Rectangle())
      .gesture(
        DragGesture(minimumDistance: 0).onChanged { drag in
          change(unit((drag.location.x - SlopPickerKnob.size / 2) / travel))
        })
    }
    .frame(height: SlopPickerKnob.size)
    .accessibilityRepresentation {
      Slider(value: Binding(get: { value }, set: change), in: 0...1) { Text(label) }
    }
  }
}

private func unit(_ value: Double) -> Double { min(max(value, 0), 1) }
