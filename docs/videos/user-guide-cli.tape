# User guide CLI demo (ecommerce-java)
#
# Record: ./docs/videos/record-user-guide-cli.sh

Output "docs/videos/user-guide-cli-no-captions.gif"
Output "docs/videos/user-guide-cli-no-captions.mp4"

Require rgctl
Require jq

Set Shell "bash"
Set FontSize 15
Set FontFamily "Menlo, Monaco, Consolas, monospace"
Set Width 1400
Set Height 800
Set Padding 14
Set Margin 20
Set MarginFill "#0d1117"
Set WindowBar Colorful
Set WindowBarSize 40
Set BorderRadius 8
Set Theme "Catppuccin Mocha"
Set TypingSpeed 22ms
Set Framerate 30
Set PlaybackSpeed 1.0
Set CursorBlink true

Hide
Type "cd rgctl-tests/ecommerce-java"
Enter
Sleep 200ms
Type "clear"
Enter
Show

Type "# discover"
Enter
Type "rgctl discover . -l java -e target --with-cfg --with-kantra"
Enter
Sleep 4s

Type "# function inventory"
Enter
Type "rgctl -f json find --type function --count-only | jq '.total'"
Enter
Sleep 2s

Type "# callers of clearCart"
Enter
Type "rgctl callers clearCart"
Enter
Sleep 2s

Type "# named communities"
Enter
Type "rgctl -f json communities list | jq '.communities[:3][] | {id, label, member_count}'"
Enter
Sleep 2s

Type "# blast radius"
Enter
Type "rgctl blast-radius 'CartService::clearCart'"
Enter
Sleep 2s

Type "# Kantra rules"
Enter
Type `rgctl -f json find --type kantrarule --limit 5 | jq '.returned'`
Enter
Sleep 2s

Type `rgctl -f json relations --edge violates --from-type kantrarule --limit 5 | jq '.returned'`
Enter
Sleep 2s
