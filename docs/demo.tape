# Frames for docs/demo.gif; run docs/demo.sh rather than vhs directly.
Output target/demo-frames/

Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 16
Set LineHeight 1.0
Set Width 1200
Set Height 760
Set TypingSpeed 40ms
Set Theme "Catppuccin Mocha"

Hide
Type "export PATH=$PWD/target/release:$PATH COLORTERM=truecolor PS1='$ '" Enter
# Warm the cache so the recording doesn't wait on the network.
Type "mcface Notch jeb_ Dinnerbone > /dev/null; clear" Enter
Show

Type "mcface Notch jeb_ Dinnerbone --name" Sleep 400ms Enter
Sleep 2.5s

Type "mcface @steve @alex @creeper @enderman -m half -s 2 --name" Sleep 400ms Enter
Sleep 3s
Type "clear" Enter

Type "mcface @sheep @pig @cow @chicken @wolf @cat @fox @panda @axolotl @bee -m half --name" Sleep 400ms Enter
Sleep 3s

Type "mcface @witch @pillager @hoglin @warden @shulker -m half --name" Sleep 400ms Enter
Sleep 3s
