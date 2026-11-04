#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#include <sys/stat.h>
#include <time.h>
#include <wayland-client.h>
#include "pointer.h"
static struct zwlr_virtual_pointer_manager_v1 *manager;
static void global(void *d,struct wl_registry *r,uint32_t name,const char *iface,uint32_t v) {
 (void)d;if(!strcmp(iface,"zwlr_virtual_pointer_manager_v1"))manager=wl_registry_bind(r,name,&zwlr_virtual_pointer_manager_v1_interface,v<2?v:2);
}
static void removed(void*d,struct wl_registry*r,uint32_t n){(void)d;(void)r;(void)n;}
static const struct wl_registry_listener listener={global,removed};
int main(int argc,char**argv){
 const char *runtime=getenv("AGENT_LAB_RUNTIME"),*xdg=getenv("XDG_RUNTIME_DIR"),*socket=getenv("WAYLAND_DISPLAY");
 char expected[256];snprintf(expected,sizeof expected,"/run/user/%u/agent-lab/koett-niri-geometry-sol",getuid());
 struct stat st;if(!runtime||strcmp(runtime,expected)||!xdg||strcmp(runtime,xdg)||!socket||strchr(socket,'/')||!strcmp(socket,"wayland-agent-lab")||stat(runtime,&st)||st.st_uid!=getuid()||(st.st_mode&077))return 2;
 struct wl_display *display=wl_display_connect(socket);if(!display)return 3;
 struct wl_registry*r=wl_display_get_registry(display);wl_registry_add_listener(r,&listener,NULL);wl_display_roundtrip(display);if(!manager)return 4;
 struct zwlr_virtual_pointer_v1*p=zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager,NULL);
 struct timespec now;clock_gettime(CLOCK_MONOTONIC,&now);uint32_t t=(uint32_t)(now.tv_sec*1000+now.tv_nsec/1000000);
 if(argc==4&&!strcmp(argv[1],"move")){
  char *end;long x=strtol(argv[2],&end,10);if(*end||x<0||x>1280)return 5;
  long y=strtol(argv[3],&end,10);if(*end||y<0||y>800)return 5;
  zwlr_virtual_pointer_v1_motion_absolute(p,t,x,y,1280,800);zwlr_virtual_pointer_v1_frame(p);
 }else if(argc==2&&!strcmp(argv[1],"click")){
  zwlr_virtual_pointer_v1_button(p,t,272,1);zwlr_virtual_pointer_v1_frame(p);
  zwlr_virtual_pointer_v1_button(p,t,272,0);zwlr_virtual_pointer_v1_frame(p);
 }else return 5;
 wl_display_roundtrip(display);zwlr_virtual_pointer_v1_destroy(p);wl_display_roundtrip(display);wl_display_disconnect(display);return 0;
}
