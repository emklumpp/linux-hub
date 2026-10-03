@echo off
setlocal enabledelayedexpansion
set IP=10.232.20.139
set PORT=5310
set REGISTER=44
set FOUND=0
set OUTPUT_FILE=modbus_scan_%date:~-4,4%%date:~-10,2%%date:~-7,2%_%time:~0,2%%time:~3,2%.txt

REM Replace colon in time (if present) for valid filename
set OUTPUT_FILE=%OUTPUT_FILE::=%

REM Create output file with header
(
  echo Modbus Scanner Results
  echo ==================================
  echo IP: %IP%
  echo Port: %PORT%
  echo Register: %REGISTER%
  echo Scan Started: %date% %time%
  echo ==================================
  echo.
) > %OUTPUT_FILE%

echo Modbus Scanner with Error Handling
echo ==================================
echo Output file: %OUTPUT_FILE%
echo.

for /L %%a in (1,1,247) do (
  set "OUTPUT="
 
  REM Run modpoll and capture output
  for /f "delims=" %%i in ('modpoll -m tcp -a %%a -r %REGISTER% -0 -t 4:int -i -p %PORT% %IP% 2^>^&1') do (
    set "OUTPUT=!OUTPUT!%%i"
  )
 
  REM Check if device responded
  echo !OUTPUT! | findstr /C:"[" >nul
  if not errorlevel 1 (
    set /a FOUND+=1
   
    REM Output to both console and file
    echo [+] Device !FOUND!: Address %%a is ACTIVE
    echo [+] Device !FOUND!: Address %%a is ACTIVE >> %OUTPUT_FILE%
   
    REM Run detailed query with correct TCP parameters
    modpoll -m tcp -a %%a -r %REGISTER% -0 -t 4:int -i -p %PORT% %IP% >> %OUTPUT_FILE%
   
    echo. >> %OUTPUT_FILE%
    echo.
  ) else (
    echo [ ] Address %%a - No response
    echo [ ] Address %%a - No response >> %OUTPUT_FILE%
  )
)

echo.
echo ==================================
echo Scan complete! Found %FOUND% device(s)
echo Results saved to: %OUTPUT_FILE%
echo ==================================

(
  echo.
  echo ==================================
  echo Scan Summary
  echo Total devices found: %FOUND%
  echo Scan completed: %date% %time%
  echo ==================================
) >> %OUTPUT_FILE%

REM Ask user if they want to open the file
echo.
set /p OPEN="Open output file? (Y/N): "
if /i "%OPEN%"=="Y" notepad %OUTPUT_FILE%

pause