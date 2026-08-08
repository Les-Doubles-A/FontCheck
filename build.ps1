$ErrorActionPreference = 'Stop'
$python = 'C:\Users\Adam_Echalard\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe'

& $python -m pip install --quiet pillow pyinstaller
& $python convert_icon.py
& $python -m PyInstaller --noconfirm --clean --onefile --windowed --name FontCheck --icon FontCheck.ico --runtime-hook rthook_tkinter.py --add-data 'FontCheck.ico;.' --add-data 'FontCheck.png;.' --add-data 'C:\Users\Adam_Echalard\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\Lib\tkinter;tkinter' --add-binary 'C:\Users\Adam_Echalard\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\DLLs\_tkinter.pyd;.' --add-data 'C:\Users\Adam_Echalard\.cache\codex-runtimes\codex-primary-runtime\dependencies\python\tcl;tcl' fontcheck.py

Write-Host "Application créée : $PWD\dist\FontCheck.exe"
