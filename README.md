# FontCheck

Application Windows locale qui affiche et filtre les polices installées. Chaque famille est rendue directement avec sa propre police.

## Générer l’application `.exe`

Dans PowerShell depuis ce dossier :

```powershell
.\build.ps1
```

Le fichier final est créé dans `dist\FontCheck.exe`. Le script convertit automatiquement `FontCheck.png` en `FontCheck.ico` et l’utilise pour l’application Windows.

## Lancer sans compiler

```powershell
C:\Users\Adam_Echalard\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe .\fontcheck.py
```
