import os
from PIL import Image, ImageDraw

def render_remote_keyboard_glyph(size, bg_color=None, padding_ratio=0.10):
    """
    Renders a crisp, Windows 11 Fluent style keyboard + wireless signal glyph.
    If bg_color is None, background is fully transparent (for tray icon & favicon).
    If bg_color is provided (e.g. (0,0,0,255)), renders solid OLED background (for iOS touch icon).
    """
    # Supersampling 4x for crystal clear anti-aliasing
    scale = 4
    canvas_size = size * scale
    
    if bg_color:
        img = Image.new("RGBA", (canvas_size, canvas_size), bg_color)
    else:
        img = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
        
    draw = ImageDraw.Draw(img)
    
    white = (255, 255, 255, 255)
    
    pad = canvas_size * padding_ratio
    w = canvas_size - 2 * pad
    h = canvas_size - 2 * pad
    cx = canvas_size / 2
    
    # Proportions:
    # Top 36% of height: Wireless / Remote signal arcs
    # Bottom 60% of height: Keyboard body
    
    stroke = max(2 * scale, int(canvas_size * 0.055))
    
    # 1. Keyboard Body
    kb_w = w * 0.90
    kb_h = h * 0.52
    kb_x0 = cx - kb_w / 2
    kb_y0 = canvas_size - pad - kb_h
    kb_x1 = cx + kb_w / 2
    kb_y1 = canvas_size - pad
    corner_r = kb_h * 0.22
    
    # Draw keyboard outline
    draw.rounded_rectangle(
        [kb_x0, kb_y0, kb_x1, kb_y1],
        radius=corner_r,
        outline=white,
        width=stroke
    )
    
    # Internal keys:
    # Row 1 dots: 4 key dots
    # Row 2 dots: 4 key dots
    # Row 3: Spacebar line
    inner_pad_x = kb_w * 0.16
    inner_w = kb_w - 2 * inner_pad_x
    
    row1_y = kb_y0 + kb_h * 0.28
    row2_y = kb_y0 + kb_h * 0.53
    row3_y = kb_y0 + kb_h * 0.77
    
    dot_r = max(scale, int(kb_w * 0.038))
    
    # 4 dots row 1
    for i in range(4):
        x = kb_x0 + inner_pad_x + (inner_w * i / 3.0)
        draw.ellipse([x - dot_r, row1_y - dot_r, x + dot_r, row1_y + dot_r], fill=white)
        
    # 4 dots row 2
    for i in range(4):
        # Slightly offset staggered row
        offset = (inner_w / 6.0) if i % 2 == 1 else 0
        x = kb_x0 + inner_pad_x + (inner_w * (i + 0.2) / 3.4)
        draw.ellipse([x - dot_r, row2_y - dot_r, x + dot_r, row2_y + dot_r], fill=white)
        
    # Spacebar line
    space_w = inner_w * 0.50
    draw.rounded_rectangle(
        [cx - space_w / 2, row3_y - stroke / 2, cx + space_w / 2, row3_y + stroke / 2],
        radius=stroke / 2,
        fill=white
    )
    
    # 2. Wireless / Remote Signal Arcs (above keyboard)
    # Origin of signal is at top center of keyboard: (cx, kb_y0)
    # Radiating upwards: 2 smooth concentric arcs
    signal_origin_y = kb_y0 + kb_h * 0.05
    
    # Outer arc
    r_outer = kb_h * 0.68
    draw.arc(
        [cx - r_outer, signal_origin_y - r_outer, cx + r_outer, signal_origin_y + r_outer],
        start=215,
        end=325,
        fill=white,
        width=stroke
    )
    
    # Inner arc
    r_inner = kb_h * 0.40
    draw.arc(
        [cx - r_inner, signal_origin_y - r_inner, cx + r_inner, signal_origin_y + r_inner],
        start=205,
        end=335,
        fill=white,
        width=stroke
    )
    
    # Small center pulse dot
    pulse_r = max(scale, int(dot_r * 1.1))
    draw.ellipse(
        [cx - pulse_r, signal_origin_y - kb_h * 0.14 - pulse_r, cx + pulse_r, signal_origin_y - kb_h * 0.14 + pulse_r],
        fill=white
    )

    # Downsample using high quality Lanczos resampling
    final_img = img.resize((size, size), Image.Resampling.LANCZOS)
    return final_img

def main():
    os.makedirs("assets", exist_ok=True)
    os.makedirs("static", exist_ok=True)
    
    # 1. Windows Multi-DPI Icon
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = []
    for s in sizes:
        img = render_remote_keyboard_glyph(s, bg_color=None, padding_ratio=0.08)
        images.append(img)
        # Also save 32px png
        if s == 32:
            img.save("static/favicon.png", format="PNG")
        if s == 48:
            img.save("assets/icon_48.png", format="PNG")
        if s == 256:
            img.save("assets/icon_256.png", format="PNG")
            
    # Save assets/icon.ico with all resolutions
    images[0].save(
        "assets/icon.ico",
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=images[1:]
    )
    print("Created assets/icon.ico (sizes: 16 to 256)")
    
    # Save static/favicon.ico
    fav_images = [img for img, s in zip(images, sizes) if s in [16, 32, 48]]
    fav_images[0].save(
        "static/favicon.ico",
        format="ICO",
        sizes=[(img.width, img.height) for img in fav_images],
        append_images=fav_images[1:]
    )
    print("Created static/favicon.ico")
    
    # 2. Apple Touch Icon & PWA Icons (on pure OLED black)
    ios_touch = render_remote_keyboard_glyph(180, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    ios_touch.save("static/apple-touch-icon.png", format="PNG")
    print("Created static/apple-touch-icon.png (180x180)")

    pwa_192 = render_remote_keyboard_glyph(192, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    pwa_192.save("static/icon-192.png", format="PNG")
    
    pwa_512 = render_remote_keyboard_glyph(512, bg_color=(0, 0, 0, 255), padding_ratio=0.18)
    pwa_512.save("static/icon-512.png", format="PNG")
    print("Created static/icon-192.png and static/icon-512.png")

if __name__ == "__main__":
    main()
