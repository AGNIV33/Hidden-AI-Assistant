# Installs local Whisper (speech-to-text) into D:\whisper for Peek AI.
# Downloads: whisper.cpp for Windows (CPU) + the large-v3-turbo q8_0 model (~834 MB).
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$Dir       = 'D:\whisper'
$ModelName = 'ggml-large-v3-turbo-q8_0.bin'
$ModelUrl  = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/$ModelName"

function Done($msg, $ok) {
  Write-Host ''
  if ($ok) { Write-Host $msg -ForegroundColor Green } else { Write-Host $msg -ForegroundColor Red }
  Read-Host 'Press Enter to close'
  if ($ok) { exit 0 } else { exit 1 }
}

try {
  if (-not (Test-Path 'D:\')) { Done 'No D: drive found on this PC.' $false }
  New-Item -ItemType Directory -Force -Path $Dir | Out-Null
  $free = (Get-PSDrive D).Free
  if ($free -lt 2GB) { Done ('Not enough space on D: (need about 2 GB, have {0:N1} GB).' -f ($free/1GB)) $false }

  # 1. Program. Windows downloads live in the nightly builds (b####) and their names change,
  #    so we search recent releases by pattern instead of exact names.
  Write-Host '1/3  Finding the whisper.cpp Windows download...' -ForegroundColor Cyan
  $all = Invoke-RestMethod 'https://api.github.com/repos/ggml-org/whisper.cpp/releases?per_page=20' -Headers @{ 'User-Agent' = 'PeekAI' }
  $isWin  = { param($n) $n -match '\.zip$' -and $n -match 'x64' -and ($n -match 'win' -or $n -match 'bin-x64') -and $n -notmatch 'arm|xcframework|ubuntu|linux|macos' }
  $isCpu  = { param($n) (& $isWin $n) -and $n -notmatch 'cuda|cublas|cudart|vulkan|blas|sycl|hip|opencl|kompute' }
  $isGpu  = { param($n) (& $isWin $n) -and $n -match 'cuda|cublas' -and $n -notmatch 'cudart' -and $n -match '12' }
  $isRt   = { param($n) (& $isWin $n) -and $n -match 'cudart' -and $n -match '12' }
  $rel = $all | Where-Object { -not $_.draft -and ($_.assets | Where-Object { & $isCpu $_.name }) } | Select-Object -First 1
  if (-not $rel) {
    $names = ($all | Select-Object -First 4 | ForEach-Object { $_.tag_name + ': ' + (($_.assets | ForEach-Object { $_.name }) -join ', ') }) -join "`n"
    Done ("No Windows build found in recent whisper.cpp releases.`nWhat GitHub returned:`n" + $names) $false
  }
  function Get-Zip($asset, $folder) {
    $zip = Join-Path $Dir 'download.zip'
    Write-Host ("     downloading {0} ({1:N0} MB)" -f $asset.name, ($asset.size/1MB))
    Invoke-WebRequest $asset.browser_download_url -OutFile $zip -UseBasicParsing
    $target = Join-Path $Dir $folder
    New-Item -ItemType Directory -Force -Path $target | Out-Null
    Expand-Archive -Path $zip -DestinationPath $target -Force
    Remove-Item $zip
    return (Get-ChildItem $target -Recurse -Filter 'whisper-server.exe' | Select-Object -First 1)
  }
  foreach ($f in 'bin-cpu','bin-gpu') { $t = Join-Path $Dir $f; if (Test-Path $t) { Remove-Item $t -Recurse -Force } }

  $cpu = Get-Zip ($rel.assets | Where-Object { & $isCpu $_.name } | Select-Object -First 1) 'bin-cpu'
  if (-not $cpu) { Done 'Downloaded the program, but whisper-server.exe was not inside it.' $false }
  Write-Host ("     OK: CPU version ({0})" -f $rel.tag_name)

  $gpu = $null
  $hasNvidia = Get-CimInstance Win32_VideoController | Where-Object { $_.Name -match 'NVIDIA' }
  $gpuAsset = $rel.assets | Where-Object { & $isGpu $_.name } | Sort-Object name -Descending | Select-Object -First 1
  if ($hasNvidia -and $gpuAsset) {
    Write-Host ("     NVIDIA card found ({0}). Getting the GPU version..." -f @($hasNvidia)[0].Name)
    $gpu = Get-Zip $gpuAsset 'bin-gpu'
    $rt = $rel.assets | Where-Object { & $isRt $_.name } | Select-Object -First 1
    if ($gpu -and $rt) {
      # CUDA runtime DLLs ship separately: put them next to whisper-server.exe
      $zip = Join-Path $Dir 'cudart.zip'
      Write-Host ("     downloading {0}" -f $rt.name)
      Invoke-WebRequest $rt.browser_download_url -OutFile $zip -UseBasicParsing
      Expand-Archive -Path $zip -DestinationPath $gpu.DirectoryName -Force
      Remove-Item $zip
    }
    if ($gpu) { Write-Host '     OK: GPU version' }
  }

  # 2. Model
  $model = Join-Path $Dir $ModelName
  if ((Test-Path $model) -and ((Get-Item $model).Length -gt 800MB)) {
    Write-Host '2/3  Model already downloaded.' -ForegroundColor Cyan
  } else {
    Write-Host '2/3  Downloading the model (about 834 MB, this can take a while)...' -ForegroundColor Cyan
    try { Start-BitsTransfer -Source $ModelUrl -Destination $model -DisplayName 'Whisper model' }
    catch { Write-Host '     (switching download method)'; Invoke-WebRequest $ModelUrl -OutFile $model -UseBasicParsing }
    if ((Get-Item $model).Length -lt 800MB) { Done 'The model download looks incomplete. Run this installer again.' $false }
  }
  Write-Host '     OK'

  # 3. Start file + desktop shortcut
  Write-Host '3/3  Creating the start file...' -ForegroundColor Cyan
  $threads = [Math]::Max(2, [Math]::Min(8, [Environment]::ProcessorCount - 2))
  $start = Join-Path $Dir 'start-whisper.bat'
  $run = '-m "{0}" -l auto -t {1} --host 127.0.0.1 --port 8080 --inference-path /v1/audio/transcriptions' -f $model, $threads
  $lines = @(
    '@echo off',
    'title Local Whisper for Peek AI - keep this window open',
    'echo Local Whisper is running. Keep this window open while you use listening mode.'
  )
  if ($gpu) {
    $lines += 'echo Trying the NVIDIA graphics card version...'
    $lines += ('"{0}" {1}' -f $gpu.FullName, $run)
    $lines += 'echo GPU version stopped. Starting the CPU version instead...'
  }
  $lines += ('"{0}" {1}' -f $cpu.FullName, $run)
  $lines += 'echo.'
  $lines += 'echo Whisper stopped. If you see a missing DLL error, install "Microsoft Visual C++ Redistributable x64".'
  $lines += 'pause'
  Set-Content -Path $start -Value $lines -Encoding ASCII
  $lnk = Join-Path ([Environment]::GetFolderPath('Desktop')) 'Start Local Whisper.lnk'
  $sh = (New-Object -ComObject WScript.Shell).CreateShortcut($lnk)
  $sh.TargetPath = $start
  $sh.WorkingDirectory = $Dir
  $sh.Save()

  Done ("Installed in $Dir`n`nNext:`n 1. Double-click 'Start Local Whisper' on your Desktop (keep that window open).`n 2. In the app: Settings > Speech-to-text > choose 'Local Whisper (this PC)'.") $true
}
catch { Done ("Install failed: " + $_.Exception.Message) $false }
