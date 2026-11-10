# FontCheck (Rust Edition)

Dépôt GitHub : **https://github.com/Les-Doubles-A/FontCheck**

Application Windows native ultra-rapide et légère écrite en **Rust** (`tao` + `wry` / WebView2 + API Windows GDI/Registre) qui affiche, filtre et gère les licences de vos polices installées en temps réel.

## Fonctionnalités

- **Moteur 100 % Rust ultra-optimisé** : Exécutable natif autonome (`LTO fat`, `codegen-units = 1`, `strip = symbols`), démarrage instantané et empreinte mémoire minimale.
- **Détection exhaustive des polices Windows** : Combine l'API native Win32 GDI (`EnumFontFamiliesExW`) et les clés de registre système (`HKLM`) et utilisateur (`HKCU`).
- **Rendu virtualisé fluide (60+ FPS)** : Pool fixe de lignes DOM avec rendu direct de chaque police.
- **Gestion des licences & Favoris** : Catégorisation persistante (`Inconnu`, `Personnel`, `Commercial`, `Libre (OFL)`) et favoris sauvegardés dans `%APPDATA%\FontCheck\`.
- **Vérification et mise à jour automatique** : Vérifie automatiquement au lancement (et à la demande) si une nouvelle version est disponible sur [GitHub Releases](https://github.com/Les-Doubles-A/FontCheck/releases) et propose de mettre à jour l'application en un clic.

## Compiler l'application `.exe`

Dans PowerShell depuis ce dossier :

```powershell
.\build.ps1
```

Ou directement avec Cargo :

```powershell
cargo build --release
```

Le binaire final optimisé est généré dans `dist\FontCheck.exe` (et `target\release\FontCheck.exe`).

<!-- installeur-release -->
## Installation

Téléchargez **`FontCheck-Setup-1.0.0.exe`** depuis la [dernière release](https://github.com/Les-Doubles-A/FontCheck/releases/latest), puis lancez-le.

L'installation se fait pour l'utilisateur courant, sans droits administrateur (par défaut dans `%LOCALAPPDATA%\Programs\FontCheck`), avec un raccourci dans le menu Démarrer et une désinstallation depuis les Paramètres Windows.

L'installeur n'est pas signé : si Windows SmartScreen l'affiche, cliquez sur « Informations complémentaires » puis « Exécuter quand même ».

### Recréer l'installeur

Compiler l'application (`cargo build --release`), puis lancer [Inno Setup 6](https://jrsoftware.org/isinfo.php) : `iscc installer\FontCheck.iss`. Le setup est créé dans `installer\Output\`.
