import os
from PIL import Image, ImageDraw, ImageFont

def render_centered_single_arc_icon(size, bg_color=None):
    scale = 4
    canvas_size = size * scale
    
    scratch_size = canvas_size * 2
    scratch = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    white = (255, 255, 255, 255)
    
    # 1. Render original Windows SegoeIcons TouchKeyboard glyph (\uE765)
    # Sized so the keyboard fills the width naturally (~88% of canvas)
    font_size = int(canvas_size * 0.90)
    font = ImageFont.truetype("C:/Windows/Fonts/SegoeIcons.ttf", font_size)
    
    kb_temp = Image.new("RGBA", (scratch_size, scratch_size), (0, 0, 0, 0))
    ImageDraw.Draw(kb_temp).text((100, 100), "\uE765", font=font, fill=white)
    bbox = kb_temp.getbbox()
    
    kb_w = bbox[2] - bbox[0]
    kb_h = bbox[3] - bbox[1]
    kb_cropped = kb_temp.crop(bbox)
    
    # Target keyboard width in final canvas = 88%
    target_kb_w = int(canvas_size * 0.88)
    target_kb_h = int(kb_h * (target_kb_w / kb_w))
    kb_scaled = kb_cropped.resize((target_kb_w, target_kb_h), Image.Resampling.LANCZOS)
    
    # Place keyboard centered horizontally, positioned at bottom leaving space for arc on top
    # Bottom margin ~ 5% of canvas
    kb_x = (canvas_size - target_kb_w) // 2
    kb_y = canvas_size - int(canvas_size * 0.05) - target_kb_h
    
    if bg_color:
        final_img = Image.new("RGBA", (canvas_size, canvas_size), bg_color)
    else:
        final_img = Image.new("RGBA", (canvas_size, canvas_size), (0, 0, 0, 0))
        
    final_img.paste(kb_scaled, (kb_x, kb_y), kb_scaled)
    final_draw = ImageDraw.Draw(final_img)
    
    # 2. Single signal arc centered above keyboard, radiating North
    cx = canvas_size // 2
    stroke = max(2 * scale, int(font_size * 0.052))
    
    # Radius so the top of the arc reaches near the top of the canvas (~8% margin)
    top_margin = int(canvas_size * 0.07)
    arc_top = top_margin
    # Arc height:
    arc_height = kb_y - arc_top
    
    # Origin of radius: just below top border of keyboard
    origin_y = kb_y + int(target_kb_h * 0.15)
    r = origin_y - arc_top
    
    # Radiate towards North: start 215 deg, end 325 deg (or 210 to 330)
    final_draw.arc([cx - r, origin_y - r, cx + r, origin_y + r],
                   start=215, end=325, fill=white, width=stroke)
                   
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
