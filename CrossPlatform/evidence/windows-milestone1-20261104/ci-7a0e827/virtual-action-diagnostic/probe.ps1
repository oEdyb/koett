Add-Type -AssemblyName System.Windows.Forms,UIAutomationProvider,UIAutomationTypes
Add-Type -ReferencedAssemblies System.Windows.Forms,System.Drawing,UIAutomationProvider,UIAutomationTypes,WindowsBase -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Windows.Forms;
using System.Runtime.InteropServices;
using System.IO;
using System.Windows.Automation;
using System.Windows.Automation.Provider;
public class NativePillProbe:Form {
 public static string LogPath;
 [DllImport("user32.dll",CharSet=CharSet.Unicode)]static extern IntPtr CreateWindowEx(uint ex,string cls,string text,uint style,int x,int y,int w,int h,IntPtr parent,IntPtr id,IntPtr instance,IntPtr param);
 [DllImport("user32.dll")]public static extern IntPtr GetForegroundWindow();
 public static void Log(string value){File.AppendAllText(LogPath,DateTime.UtcNow.ToString("O")+" "+value+" fg="+GetForegroundWindow().ToInt64()+"\r\n");}
 protected override bool ShowWithoutActivation {get{return true;}}
 protected override CreateParams CreateParams {get {var c=base.CreateParams;c.ExStyle|=0x08000000;return c;}}
 List<ProbeButton> hooks=new List<ProbeButton>();
 protected override void OnShown(EventArgs e){base.OnShown(e);for(int i=0;i<1;i++){
 var h=CreateWindowEx(0x08000000,"BUTTON",i==0?"Default Invoke":"Direct Invoke",0x5000000b,20+i*200,20,180,40,Handle,(IntPtr)(100+i),IntPtr.Zero,IntPtr.Zero);
 var hook=new ProbeButton(i==1,Handle,100+i);hook.AssignHandle(h);hooks.Add(hook);}
 }
 protected override void WndProc(ref Message m){
 if(m.Msg==0x3d&&(int)m.LParam.ToInt64()==AutomationInteropProvider.RootObjectId){m.Result=AutomationInteropProvider.ReturnRawElementProvider(Handle,m.WParam,m.LParam,new RootProvider(Handle));return;}
 if(m.Msg==0x21){m.Result=(IntPtr)3;return;}
 if(m.Msg==6||m.Msg==7||m.Msg==0x111)Log("parent msg="+m.Msg+" w="+m.WParam.ToInt64()+" l="+m.LParam.ToInt64());
 if(m.Msg==0x111){m.Result=IntPtr.Zero;return;}
 if(m.Msg==0x2b){m.Result=(IntPtr)1;return;}
 base.WndProc(ref m);
 }
}
public class RootProvider:IRawElementProviderSimple,IRawElementProviderFragmentRoot {
 public IntPtr hwnd;
 public RootProvider(IntPtr h){hwnd=h;}
 public ProviderOptions ProviderOptions {get{return ProviderOptions.ServerSideProvider|ProviderOptions.ProviderOwnsSetFocus;}}
 public IRawElementProviderSimple HostRawElementProvider {get{return AutomationInteropProvider.HostProviderFromHandle(hwnd);}}
 public object GetPatternProvider(int p){return null;}
 public object GetPropertyValue(int p){if(p==AutomationElementIdentifiers.NameProperty.Id)return "Native Pill Probe";if(p==AutomationElementIdentifiers.ControlTypeProperty.Id)return ControlType.Pane.Id;if(p==AutomationElementIdentifiers.IsKeyboardFocusableProperty.Id)return false;return null;}
 public IRawElementProviderFragment Navigate(NavigateDirection d){if(d==NavigateDirection.FirstChild||d==NavigateDirection.LastChild)return new ActionProvider(this);return null;}
 public int[] GetRuntimeId(){return null;}
 public IRawElementProviderSimple[] GetEmbeddedFragmentRoots(){return null;}
 [StructLayout(LayoutKind.Sequential)]struct R{public int l,t,r,b;}
 [DllImport("user32.dll")]static extern bool GetWindowRect(IntPtr h,out R r);
 public System.Windows.Rect BoundingRectangle {get{R r;GetWindowRect(hwnd,out r);return new System.Windows.Rect(r.l,r.t,r.r-r.l,r.b-r.t);}}
 public void SetFocus(){NativePillProbe.Log("root SetFocus no activation");}
 public IRawElementProviderFragmentRoot FragmentRoot {get{return this;}}
 public IRawElementProviderFragment ElementProviderFromPoint(double x,double y){return new ActionProvider(this);}
 public IRawElementProviderFragment GetFocus(){return null;}
}
public class ActionProvider:IRawElementProviderSimple,IInvokeProvider,IRawElementProviderFragment {
 RootProvider root;
 [DllImport("user32.dll")]static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 public ActionProvider(RootProvider r){root=r;}
 public ProviderOptions ProviderOptions {get{return ProviderOptions.ServerSideProvider|ProviderOptions.ProviderOwnsSetFocus;}}
 public IRawElementProviderSimple HostRawElementProvider {get{return null;}}
 public object GetPatternProvider(int p){return p==InvokePatternIdentifiers.Pattern.Id?this:null;}
 public object GetPropertyValue(int p){if(p==AutomationElementIdentifiers.IsKeyboardFocusableProperty.Id)return false;if(p==AutomationElementIdentifiers.HasKeyboardFocusProperty.Id)return false;if(p==AutomationElementIdentifiers.ControlTypeProperty.Id)return ControlType.Button.Id;if(p==AutomationElementIdentifiers.NameProperty.Id)return "Direct Invoke";return null;}
 public IRawElementProviderFragment Navigate(NavigateDirection d){return d==NavigateDirection.Parent?root:null;}
 public int[] GetRuntimeId(){return new int[]{AutomationInteropProvider.AppendRuntimeId,101};}
 public IRawElementProviderSimple[] GetEmbeddedFragmentRoots(){return null;}
 public System.Windows.Rect BoundingRectangle {get{var r=root.BoundingRectangle;return new System.Windows.Rect(r.X+220,r.Y+50,180,40);}}
 public void SetFocus(){NativePillProbe.Log("virtual SetFocus no activation");}
 public IRawElementProviderFragmentRoot FragmentRoot {get{return root;}}
 public void Invoke(){NativePillProbe.Log("virtual invoke");PostMessage(root.hwnd,0x111,(IntPtr)101,IntPtr.Zero);}
}
public class ProbeButton:NativeWindow {
 bool direct;IntPtr parent;int id;
 [DllImport("user32.dll")]static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
 public ProbeButton(bool d,IntPtr p,int i){direct=d;parent=p;id=i;}
 protected override void WndProc(ref Message m){
 if(m.Msg==0xf5||m.Msg==6||m.Msg==7||m.Msg==0x201||m.Msg==0x202)NativePillProbe.Log("button "+id+" msg="+m.Msg);
 if(direct&&m.Msg==0x3d&&m.LParam.ToInt64()==AutomationInteropProvider.RootObjectId){m.Result=AutomationInteropProvider.ReturnRawElementProvider(Handle,m.WParam,m.LParam,new ActionProvider(new RootProvider(parent)));return;}
 if(direct&&m.Msg==0xf5){PostMessage(parent,0x111,(IntPtr)id,Handle);m.Result=IntPtr.Zero;return;}
 base.WndProc(ref m);
 }
}

'@
[NativePillProbe]::LogPath=Join-Path $env:USERPROFILE 'native-pill-probe.log'
$f=New-Object NativePillProbe;$f.Text='Native Pill Probe';$f.Width=480;$f.Height=150;[void]$f.ShowDialog()
