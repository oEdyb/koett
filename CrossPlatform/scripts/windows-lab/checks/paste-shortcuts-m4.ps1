$ErrorActionPreference='Stop'
$config=$env:APPDATA+'\DybergCo\Koett\config\settings.json'
$original=[IO.File]::ReadAllText($config)
$window=[LabDriver]::ByTitle('Koett Settings')
if($window-eq[IntPtr]::Zero){Send-DictationCommand 5;Start-Sleep -Milliseconds 300;$window=[LabDriver]::ByTitle('Koett Settings')}
Activate-LabEditor $window
function Menu-Item([string]$name){
 $menu=[LabDriver]::ByClass('#32768');if($menu-eq[IntPtr]::Zero){throw 'Native choice menu missing'}
 $root=[Windows.Automation.AutomationElement]::FromHandle($menu)
 $item=$root.FindFirst([Windows.Automation.TreeScope]::Descendants,(New-Object Windows.Automation.PropertyCondition ([Windows.Automation.AutomationElement]::NameProperty),$name))
 if(!$item){throw ('Accessible native menu choice missing: '+$name)}
 $item.GetCurrentPattern([Windows.Automation.InvokePattern]::Pattern).Invoke();Start-Sleep -Milliseconds 120
}
function Choose([string]$group,[string]$chord){
 $null=Invoke-LabButton $window 'Paste shortcuts…';Menu-Item $group;Menu-Item $chord
 [IO.File]::ReadAllText($config)|ConvertFrom-Json
}
$rows=@()
try{
 $text=Choose 'Text' 'Ctrl+Shift+V';$rows+=@{step='text';value=[string]$text.paste_shortcut;passed=($text.paste_shortcut-eq'ctrl_shift_v')}
 $default=Choose 'Images — default' 'Ctrl+Shift+V';$rows+=@{step='default-image';value=[string]$default.image_paste_shortcuts.'windows:*';passed=($default.image_paste_shortcuts.'windows:*'-eq'ctrl_shift_v')}
 $app=Choose 'Images — msedge' 'Ctrl+V';$rows+=@{step='app-image';value=[string]$app.image_paste_shortcuts.'windows:msedge.exe';passed=($app.image_paste_shortcuts.'windows:msedge.exe'-eq'ctrl_v'-and$app.image_paste_shortcuts.'windows:*'-eq'ctrl_shift_v')}
 $inherited=Choose 'Images — msedge' 'Use default';$rows+=@{step='inherit-image';passed=(!($inherited.image_paste_shortcuts.PSObject.Properties.Name-contains'windows:msedge.exe')-and$inherited.image_paste_shortcuts.'windows:*'-eq'ctrl_shift_v')}
}finally{
 [IO.File]::WriteAllText($config,$original,(New-Object Text.UTF8Encoding($false)))
}
@{passed=(@($rows|Where-Object {-not$_.passed}).Count-eq0);rows=$rows;focus=Get-LabFocus;artifact=Get-LabArtifact;scope='UIA invokes actual native choices; guest preferences restored. Physical keyboard cancellation is checked separately.'}
