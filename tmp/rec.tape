Output tmp/rec.mp4
Set FontSize 14
Set Width 1200
Set Height 700
Type "grep -E \"reply::.*(reads_how|past_mail|this_conversation|first_message|my_own|suggestions_get|length_of|matches_the)\" tmp/passed.txt"
Enter
Sleep 3s
Type "clear && cat tmp/demo.json"
Enter
Sleep 4s
Type "clear && git show --stat --format=%s HEAD~1 | head -20"
Enter
Sleep 4s
