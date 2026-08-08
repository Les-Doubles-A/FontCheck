"""Rend le dossier tkinter embarqué importable avant le lancement de l'application."""
import sys

if getattr(sys, "frozen", False):
    sys.path.insert(0, sys._MEIPASS)
