@echo off
echo === CC Proxy Build Script ===
echo.

where pnpm >nul 2>&1 || (echo pnpm required & exit /b 1)
where cargo >nul 2>&1 || (echo cargo required & exit /b 1)

pnpm install
pnpm tauri build

echo.
echo === Build Complete ===
echo Output: src-tauri\target\release\bundle\
dir /s /b src-tauri\target\release\bundle\*.exe src-tauri\target\release\bundle\*.msi 2>nul
pause
