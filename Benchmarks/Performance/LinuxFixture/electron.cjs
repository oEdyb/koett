const {app,BrowserWindow}=require('electron');
app.setPath('userData',process.env.HOME+'/.profiles/perf');
app.whenReady().then(()=>{const w=new BrowserWindow({width:1300,height:900,title:'Koett performance receiver',webPreferences:{nodeIntegration:true,contextIsolation:false}});w.setMenuBarVisibility(false);w.loadFile(__dirname+'/receiver.html');});
app.on('window-all-closed',()=>app.quit());
