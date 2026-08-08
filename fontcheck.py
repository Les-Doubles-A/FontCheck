"""FontCheck -- fast local Windows font browser."""
from __future__ import annotations

import ctypes
import json
import os
import sys
import tkinter as tk
from pathlib import Path
from tkinter import font as tkfont

APP_DIR = Path(getattr(sys, "_MEIPASS", Path(__file__).resolve().parent))
BG, PAPER, INK, MUTED, LINE, ACCENT = "#f4efe6", "#fffdfa", "#20201e", "#75716b", "#d9d2c7", "#e95c43"

# --- Licence des polices ----------------------------------------------------
# Chaque police peut être marquée avec une de ces catégories. "inconnu" est la
# valeur par défaut tant que l'utilisateur n'a rien choisi.
LICENSE_ORDER = ["inconnu", "personnel", "commercial", "libre"]
LICENSES = {
    "inconnu":    {"label": "Inconnu",        "code": "?",    "color": MUTED},
    "personnel":  {"label": "Personnel",      "code": "PERS", "color": "#c9463a"},
    "commercial": {"label": "Commercial",     "code": "COMM", "color": "#2f8f57"},
    "libre":      {"label": "Libre (OFL)",    "code": "OFL",  "color": "#3a6fc9"},
}


def data_dir() -> Path:
    """Dossier persistant (hors du .exe onefile, qui est éphémère)."""
    base = Path(os.environ.get("APPDATA", str(Path.home())))
    directory = base / "FontCheck"
    directory.mkdir(parents=True, exist_ok=True)
    return directory


def licenses_path() -> Path:
    return data_dir() / "licenses.json"


def load_licenses() -> dict[str, str]:
    path = licenses_path()
    if not path.exists():
        return {}
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
        return {family: status for family, status in data.items() if status in LICENSES}
    except (OSError, json.JSONDecodeError):
        return {}


def save_licenses(data: dict[str, str]) -> None:
    try:
        licenses_path().write_text(json.dumps(data, ensure_ascii=False, indent=2), encoding="utf-8")
    except OSError:
        pass


def load_logo(size: int) -> tk.PhotoImage | None:
    """Charge FontCheck.png redimensionné à `size` px (carré). None si absent."""
    path = APP_DIR / "FontCheck.png"
    if not path.exists():
        return None
    try:
        from PIL import Image, ImageTk
        image = Image.open(path).convert("RGBA")
        image = image.resize((size, size), Image.LANCZOS)
        return ImageTk.PhotoImage(image)
    except ImportError:
        try:
            raw = tk.PhotoImage(file=str(path))
            factor = max(1, raw.width() // size)
            return raw.subsample(factor, factor) if factor > 1 else raw
        except tk.TclError:
            return None
    except Exception:
        return None


def windows_fonts() -> list[str]:
    """Returns available font families, including Windows registered fonts."""
    families = set(tkfont.families())
    try:
        import winreg
        with winreg.OpenKey(winreg.HKEY_LOCAL_MACHINE, r"SOFTWARE\Microsoft\Windows NT\CurrentVersion\Fonts") as key:
            index = 0
            while True:
                try:
                    label, _, _ = winreg.EnumValue(key, index)
                    family = label.removesuffix(" (TrueType)").removesuffix(" (OpenType)")
                    for suffix in (" Bold Italic", " Bold", " Italic", " Regular"):
                        if family.endswith(suffix):
                            family = family[:-len(suffix)]
                    families.add(family)
                    index += 1
                except OSError:
                    break
    except OSError:
        pass
    return sorted((item for item in families if item and not item.startswith("@")), key=str.casefold)


class FontCheck(tk.Tk):
    """Uses a fixed pool of rows rather than one widget per font."""
    def __init__(self) -> None:
        super().__init__()
        self.title("FontCheck")
        self.geometry("1120x760")
        self.minsize(760, 530)
        self.configure(bg=BG)
        self.fonts = windows_fonts()
        self.filtered_fonts = list(self.fonts)
        self.favorites: set[str] = set()
        self.licenses: dict[str, str] = load_licenses()
        self.license_filter = "toutes"
        self.sample = tk.StringVar(value="Portez ce vieux whisky au juge blond qui fume.")
        self.query = tk.StringVar()
        self.size, self.row_height = 27, 105
        self.row_views: list[dict[str, object]] = []
        self.font_cache: dict[tuple[str, int], tkfont.Font] = {}
        self.render_timer: str | None = None
        self._logo_images: list[tk.PhotoImage] = []  # garde une référence, sinon Tk garbage-collect les images
        self._set_icon()
        self._build()
        self.apply_filter()

    def _set_icon(self) -> None:
        icon = APP_DIR / "FontCheck.ico"
        if icon.exists():
            self.iconbitmap(default=str(icon))
        # iconphoto en complément : couvre la barre des tâches / Alt-Tab sur les
        # configurations où le .ico seul n'est pas repris (et servira aussi aux
        # futures fenêtres Toplevel grâce à default=True). Plusieurs tailles sont
        # fournies pour que Windows choisisse la plus nette selon le contexte
        # (petite icône barre de titre, grande icône barre des tâches/Alt-Tab).
        photos = [load_logo(size) for size in (16, 32, 48, 64, 128, 256)]
        photos = [photo for photo in photos if photo is not None]
        if photos:
            self._logo_images.extend(photos)
            self.iconphoto(True, *photos)

    def logo(self, size: int) -> tk.PhotoImage | None:
        """Logo mis en cache par taille pour être réutilisé dans l'UI."""
        photo = load_logo(size)
        if photo is not None:
            self._logo_images.append(photo)
        return photo

    def license_of(self, family: str) -> str:
        return self.licenses.get(family, "inconnu")

    def _build(self) -> None:
        sidebar = tk.Frame(self, bg=INK, width=250); sidebar.pack(side="left", fill="y"); sidebar.pack_propagate(False)
        brand = tk.Frame(sidebar, bg=INK); brand.pack(anchor="w", padx=25, pady=30, fill="x")
        sidebar_logo = self.logo(34)
        if sidebar_logo is not None:
            tk.Label(brand, image=sidebar_logo, bg=INK).pack(side="left", padx=(0, 10))
            tk.Label(brand, text="FontCheck", font=("Segoe UI", 19, "bold"), bg=INK, fg="#fffaf3").pack(side="left")
        else:
            tk.Label(brand, text="Fc  FontCheck", font=("Segoe UI", 19, "bold"), bg=INK, fg="#fffaf3").pack(side="left")
        self.total_label = tk.Label(sidebar, bg=INK, fg="#d8d1c8", font=("Consolas", 10), justify="left"); self.total_label.pack(anchor="w", padx=27, pady=(27, 3))
        tk.Label(sidebar, text="BIBLIOTHEQUE LOCALE", font=("Consolas", 9), bg=INK, fg="#9c968d").pack(anchor="w", padx=27)

        tk.Label(sidebar, text="LICENCE", font=("Consolas", 9), bg=INK, fg="#9c968d").pack(anchor="w", padx=27, pady=(28, 8))
        filters = tk.Frame(sidebar, bg=INK); filters.pack(anchor="w", padx=27, fill="x")
        self.filter_buttons: dict[str, tk.Button] = {}
        for key, label in [("toutes", "Toutes"), *[(k, v["label"]) for k, v in LICENSES.items()]]:
            btn = tk.Button(filters, text=label, anchor="w", relief="flat", bg=INK, fg="#d8d1c8",
                             activebackground=ACCENT, activeforeground="white", font=("Consolas", 9),
                             cursor="hand2", command=lambda k=key: self.set_license_filter(k))
            btn.pack(fill="x", pady=1)
            self.filter_buttons[key] = btn

        tk.Label(sidebar, text="The details are not the details.\nThey make the design.", font=("Georgia", 13), bg=INK, fg="#f2ede4", justify="left", wraplength=190).pack(anchor="sw", side="bottom", padx=27, pady=(0, 55))
        tk.Label(sidebar, text="-- CHARLES EAMES", font=("Consolas", 8), bg=INK, fg="#9c968d").pack(anchor="sw", side="bottom", padx=27, pady=(0, 25))

        content = tk.Frame(self, bg=BG); content.pack(side="left", fill="both", expand=True)
        head = tk.Frame(content, bg=BG); head.pack(fill="x", padx=55, pady=(52, 24))
        title_row = tk.Frame(head, bg=BG); title_row.pack(anchor="w", fill="x")
        header_logo = self.logo(40)
        if header_logo is not None:
            tk.Label(title_row, image=header_logo, bg=BG).pack(side="left", padx=(0, 14))
        title_col = tk.Frame(title_row, bg=BG); title_col.pack(side="left", anchor="w")
        tk.Label(title_col, text="VOS POLICES,", font=("Segoe UI", 33, "bold"), bg=BG, fg=INK).pack(anchor="w")
        tk.Label(title_col, text="sans chercher.", font=("Georgia", 33, "italic"), bg=BG, fg=INK).pack(anchor="w")
        tk.Button(head, text="Actualiser", command=self.refresh, bg=INK, fg="white", activebackground=ACCENT, activeforeground="white", relief="flat", padx=16, pady=9, font=("Segoe UI", 10, "bold"), cursor="hand2").pack(anchor="e", side="right", pady=12)

        search_bar = tk.Frame(content, bg=BG); search_bar.pack(fill="x", padx=55)
        entry = tk.Entry(search_bar, textvariable=self.query, font=("Segoe UI", 12), relief="flat", bg=PAPER, fg=INK, insertbackground=INK)
        entry.pack(side="left", fill="x", expand=True, ipady=11, padx=(0, 10)); entry.configure(highlightthickness=1, highlightbackground=LINE, highlightcolor=ACCENT)
        entry.bind("<KeyRelease>", lambda _event: self.schedule_filter())
        tk.Button(search_bar, text="Aa", command=self.toggle_size, bg=BG, fg=INK, relief="flat", font=("Georgia", 15), cursor="hand2").pack(side="right")
        tk.Label(content, text="TEXTE D'APERCU", bg=BG, fg=ACCENT, font=("Consolas", 9, "bold")).pack(anchor="w", padx=55, pady=(28, 4))
        sample_entry = tk.Entry(content, textvariable=self.sample, font=("Georgia", 15), relief="flat", bg=BG, fg=INK, insertbackground=INK)
        sample_entry.pack(fill="x", padx=55, pady=(0, 18)); sample_entry.bind("<KeyRelease>", lambda _event: self.schedule_preview())
        self.status = tk.Label(content, bg=BG, fg=MUTED, font=("Consolas", 9), anchor="w"); self.status.pack(fill="x", padx=55, pady=(0, 4))

        holder = tk.Frame(content, bg=BG); holder.pack(fill="both", expand=True, padx=55, pady=(0, 30))
        self.canvas = tk.Canvas(holder, bg=BG, highlightthickness=0)
        scroll = tk.Scrollbar(holder, orient="vertical", command=self.scroll_rows)
        self.canvas.configure(yscrollcommand=scroll.set)
        self.canvas.pack(side="left", fill="both", expand=True); scroll.pack(side="right", fill="y")
        self.canvas.bind("<Configure>", lambda _event: self.render_visible())
        self.canvas.bind_all("<MouseWheel>", self.mouse_scroll)
        for _ in range(16):
            row = tk.Frame(self.canvas, bg=BG, height=self.row_height); row.pack_propagate(False)
            top = tk.Frame(row, bg=BG); top.pack(fill="x", padx=2, pady=(15, 0))
            name = tk.Label(top, bg=BG, fg=INK, font=("Consolas", 10, "bold")); name.pack(side="left")
            heart = tk.Button(top, bg=BG, relief="flat", font=("Segoe UI", 15), cursor="hand2"); heart.pack(side="right")
            license_badge = tk.Button(top, bg=BG, relief="flat", font=("Consolas", 9, "bold"), cursor="hand2",
                                       padx=8, pady=1, bd=1)
            license_badge.pack(side="right", padx=(0, 10))
            preview = tk.Label(row, bg=BG, fg=INK, anchor="w", justify="left"); preview.pack(fill="x", padx=2, pady=(5, 0))
            tk.Frame(row, bg=LINE, height=1).pack(side="bottom", fill="x")
            window = self.canvas.create_window(0, 0, window=row, anchor="nw", state="hidden")
            self.row_views.append({"name": name, "heart": heart, "license": license_badge, "preview": preview, "window": window})
        entry.focus_set()
        self.refresh_filter_buttons()

    def refresh_filter_buttons(self) -> None:
        for key, btn in self.filter_buttons.items():
            active = key == self.license_filter
            btn.configure(bg=ACCENT if active else INK, fg="white" if active else "#d8d1c8")

    def set_license_filter(self, key: str) -> None:
        self.license_filter = key
        self.refresh_filter_buttons()
        self.apply_filter()

    def schedule_filter(self) -> None:
        if self.render_timer: self.after_cancel(self.render_timer)
        self.render_timer = self.after(180, self.apply_filter)

    def schedule_preview(self) -> None:
        if self.render_timer: self.after_cancel(self.render_timer)
        self.render_timer = self.after(120, self.render_visible)

    def apply_filter(self) -> None:
        self.render_timer = None
        term = self.query.get().strip().casefold()
        self.filtered_fonts = [
            family for family in self.fonts
            if term in family.casefold()
            and (self.license_filter == "toutes" or self.license_of(family) == self.license_filter)
        ]
        total = len(self.filtered_fonts)
        self.canvas.configure(scrollregion=(0, 0, 1, total * self.row_height)); self.canvas.yview_moveto(0)
        self.status.configure(text=f"{total} police(s) affichee(s)")
        self.total_label.configure(text=f"{len(self.fonts)} POLICES\nINSTALLEES")
        self.render_visible()

    def render_visible(self) -> None:
        self.render_timer = None
        start = max(0, int(self.canvas.canvasy(0) // self.row_height) - 1)
        text = self.sample.get() or "Votre texte ici"
        for slot, view in enumerate(self.row_views):
            index = start + slot
            if index >= len(self.filtered_fonts):
                self.canvas.itemconfigure(view["window"], state="hidden")
                continue
            family = self.filtered_fonts[index]
            self.canvas.itemconfigure(view["window"], state="normal", width=self.canvas.winfo_width())
            self.canvas.coords(view["window"], 0, index * self.row_height)
            view["name"].configure(text=family)
            heart = view["heart"]
            heart.configure(text="[x]" if family in self.favorites else "[ ]", fg=ACCENT if family in self.favorites else MUTED, command=lambda item=family: self.favorite(item))
            status = self.license_of(family)
            meta = LICENSES[status]
            badge = view["license"]
            badge.configure(text=meta["code"], fg=meta["color"], highlightbackground=meta["color"],
                             command=lambda item=family: self.cycle_license(item))
            key = (family, self.size)
            if key not in self.font_cache:
                try: self.font_cache[key] = tkfont.Font(family=family, size=self.size)
                except tk.TclError: self.font_cache[key] = tkfont.Font(family="Arial", size=self.size)
            view["preview"].configure(text=text, font=self.font_cache[key])

    def scroll_rows(self, *args: str) -> None:
        self.canvas.yview(*args); self.after_idle(self.render_visible)

    def mouse_scroll(self, event) -> str:
        if event.delta:
            self.canvas.yview_scroll(int(-event.delta / 120), "units"); self.after_idle(self.render_visible)
        return "break"

    def favorite(self, family: str) -> None:
        self.favorites.symmetric_difference_update({family}); self.render_visible()

    def cycle_license(self, family: str) -> None:
        current = self.license_of(family)
        next_status = LICENSE_ORDER[(LICENSE_ORDER.index(current) + 1) % len(LICENSE_ORDER)]
        if next_status == "inconnu":
            self.licenses.pop(family, None)
        else:
            self.licenses[family] = next_status
        save_licenses(self.licenses)
        self.render_visible()

    def toggle_size(self) -> None:
        self.size = {27: 34, 34: 21, 21: 27}[self.size]; self.render_visible()

    def refresh(self) -> None:
        self.fonts = windows_fonts(); self.apply_filter()


if __name__ == "__main__":
    try: ctypes.windll.shcore.SetProcessDpiAwareness(1)
    except Exception: pass
    # Donne à l'appli une identité Windows distincte de python.exe/pythonw.exe :
    # sans ça, Windows peut regrouper la fenêtre sous l'icône générique Python
    # dans la barre des tâches au lieu d'utiliser l'icône de FontCheck.
    try: ctypes.windll.shell32.SetCurrentProcessExplicitAppUserModelID("FontCheck.App")
    except Exception: pass
    FontCheck().mainloop()