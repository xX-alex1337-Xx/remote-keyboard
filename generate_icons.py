import os
from PIL import Image, ImageDraw, ImageFont

def render_centered_single_arc_icon(size, bg_color=None):
    scale = 4
    canvas_size = size * scale
    
    # 1. Render genuine Windows Touch Keyboard glyph (\uE765) from segmdl2.ttf
    # with sharp rectangular keys and spacebar
    font_size = int(canvas_size * 0.85)
    font = ImageFont.truetype("C:/Windows/Fonts/segmdl2.ttf", font_size)
    
    scratch_size = canvas_size * 2
    temp = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    ImageDraw.Draw(temp).text((50, 50), "\uE765", font=font, fill=(255, 255, 255, 255))
    bbox = temp.getbbox()
    kb = temp.crop(bbox)
    
    # Keyboard width = 84% of canvas
    target_kb_w = int(canvas_size * 0.84)
    target_kb_h = int(kb.height * (target_kb_w / kb.width))
    kb_scaled = kb.resize((target_kb_w, target_kb_h), Image.Resampling.LANCZOS)
    
    if bg_color:
        final_img = Image.new("RGBA", (canvas_size, canvas_size), bg_color)
    else:
        final_img = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
        
    # Keyboard positioned centered horizontally, resting slightly toward bottom
    kb_x = (canvas_size - target_kb_w) // 2
    kb_y = canvas_size - target_kb_h - int(canvas_size * 0.08)
    final_img.paste(kb_scaled, (kb_x, kb_y), kb_scaled)
    
    final_draw = ImageDraw.Draw(final_img)
    cx = canvas_size // 2
    stroke = max(2 * scale, int(canvas_size * 0.052))
    
    # 2. Single shallow North signal wave, floating high with clear separation
    # Arc angle: 244 to 296 deg (52 degrees) - gentle curve that radiates signal,
    # completely detached so it never resembles a handle or suitcase!
    r = int(canvas_size * 0.35)
    cy = int(canvas_size * 0.07) + r
    final_draw.arc([cx - r, cy - r, cx + r, cy + r],
                   start=244, end=296, fill=(255, 255, 255, 255), width=stroke)
                   
    return final_img.resize((size, size), Image.Resampling.LANCZOS)

def main():
    os.makedirs("assets", exist_ok=True)
    os.makedirs("static", exist_ok=True)
    
    sizes = [16, 20, 24, 32, 48, 64, 128, 256]
    images = []
    for s in sizes:
        img = render_centered_single_arc_icon(s, bg_color=None)
        images.append(img)
        if s == 24:
            img.save("assets/icon_24.png", format="PNG")
        if s == 32:
            img.save("static/favicon.png", format="PNG")
        if s == 48:
            img.save("assets/icon_48.png", format="PNG")
        if s == 256:
            img.save("assets/icon_256.png", format="PNG")
            
    images[0].save(
        "assets/icon.ico",
        format="ICO",
        sizes=[(s, s) for s in sizes],
        append_images=images[1:]
    )
    print("Created assets/icon.ico")
    
    fav_images = [img for img, s in zip(images, sizes) if s in [16, 32, 48]]
    fav_images[0].save(
        "static/favicon.ico",
        format="ICO",
        sizes=[(img.width, img.height) for img in fav_images],
        append_images=fav_images[1:]
    )
    print("Created static/favicon.ico")
    
    ios_touch = render_centered_single_arc_icon(180, bg_color=(0, 0, 0, 255))
    ios_touch.save("static/apple-touch-icon.png", format="PNG")
    print("Created static/apple-touch-icon.png")

    pwa_192 = render_centered_single_arc_icon(192, bg_color=(0, 0, 0, 255))
    pwa_192.save("static/icon-192.png", format="PNG")
    
    pwa_512 = render_centered_single_arc_icon(512, bg_color=(0, 0, 0, 255))
    pwa_512.save("static/icon-512.png", format="PNG")
    print("Created static/icon-192.png and static/icon-512.png")

if __name__ == "__main__":
    main()
