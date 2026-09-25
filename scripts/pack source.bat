robocopy Q:\morphosis\code\sparq %TEMP%\sparq-src /E /XD target .git
powershell -Command "Compress-Archive -Path $env:TEMP\sparq-src\* -DestinationPath $env:USERPROFILE\Desktop\sparq-tree.zip"