$ErrorActionPreference='Stop'
$browser=[Windows.Automation.AutomationElement]::RootElement.FindFirst([Windows.Automation.TreeScope]::Children,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::ClassNameProperty,'Chrome_WidgetWin_1')))
$field=$browser.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition([Windows.Automation.AutomationElement]::NameProperty,'Media focus editor')));$field.SetFocus()
$media=$env:APPDATA+'\DybergCo\Koett\data\Media Transcripts'
$global:M5FocusedBefore=@{focus=Get-LabFocus;names=@(Get-ChildItem $media -File|Select-Object -ExpandProperty Name);last=(Get-FileHash ($env:APPDATA+'\DybergCo\Koett\data\Last Transcript.txt')).Hash;history=(Get-FileHash ($env:APPDATA+'\DybergCo\Koett\data\History.sqlite3')).Hash}
$global:M5FocusedBefore
