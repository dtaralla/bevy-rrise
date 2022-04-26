@echo off

REM Copyright (c) 2022 Contributors to the Rrise project

cargo doc --no-deps
rmdir /q /s docs
echo ^<meta http-equiv="refresh" content="0; url=bevy_rrise/index.html"^>> target\doc\index.html
xcopy /q /s /e target\doc docs\
