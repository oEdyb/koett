#include <AudioToolbox/AudioToolbox.h>
#include <objc/runtime.h>
#include <objc/message.h>
#include <stdatomic.h>
#include <stdlib.h>
#include <string.h>
#include <math.h>
#include <stdio.h>
#include <sys/sysctl.h>
static float *fixture=NULL;
static uint64_t fixture_count=0;
static double fixture_rate=16000;
static _Atomic uint64_t cursor=0;
static _Atomic uint64_t finished_at=UINT64_MAX;
static AudioStreamBasicDescription old_format;
static _Atomic uint32_t input_rate=0;
double KoettAuditFixtureInputRate(void){return atomic_load(&input_rate);}
void KoettAuditSetFixture(const float *samples,uint64_t count,double rate) {
 free(fixture);fixture=malloc(count*sizeof(float));memcpy(fixture,samples,count*sizeof(float));fixture_count=count;fixture_rate=rate;
 atomic_store(&cursor,0);atomic_store(&finished_at,UINT64_MAX);
}
int KoettAuditFixtureFinished(void){return atomic_load(&cursor)>=atomic_load(&finished_at);}
uint64_t KoettAuditFixtureFrames(void){return atomic_load(&cursor);}
void KoettAuditFill(AudioBufferList *data,uint32_t frames,double rate) {
 atomic_store(&input_rate,(uint32_t)rate);
 uint64_t start=atomic_load(&cursor);
 uint64_t end=(uint64_t)ceil((0.3+(double)fixture_count/fixture_rate)*rate);
 atomic_store(&finished_at,end);
 for(uint32_t b=0;b<data->mNumberBuffers;b++) {
  AudioBuffer *buffer=&data->mBuffers[b];float *out=buffer->mData;
  if(!out) continue;
  for(uint32_t f=0;f<frames;f++) {
   double p=((double)(start+f)/rate-0.3)*fixture_rate;float value=0;
   if(fixture && p>=0 && p<(double)fixture_count) {
    uint64_t i=(uint64_t)p;double fraction=p-i;
    value=fixture[i]*(1-fraction)+(i+1<fixture_count?fixture[i+1]:0)*fraction;
   }
   for(uint32_t c=0;c<buffer->mNumberChannels;c++) out[f*buffer->mNumberChannels+c]=value;
  }
 }
 atomic_store(&cursor,start+frames);
}
static void old_callback(void *context,AudioQueueProcessingTapRef tap,UInt32 frames,AudioTimeStamp *time,AudioQueueProcessingTapFlags *flags,UInt32 *outFrames,AudioBufferList *data) {
 OSStatus status=AudioQueueProcessingTapGetSourceAudio(tap,frames,time,flags,outFrames,data);
 if(status) {*outFrames=0;return;}
 KoettAuditFill(data,*outFrames,old_format.mSampleRate);
}
int KoettAuditInstallRecorderTap(id recorder) {
 char build[64]={0};size_t length=sizeof(build);
 if(sysctlbyname("kern.osversion",build,&length,NULL,0) || strcmp(build,"25G83")) return -1;
 void *impl=((void *(*)(id,SEL))objc_msgSend)(recorder,sel_registerName("impl"));AudioQueueRef queue=NULL;
 // Harness-only layout established by a stopped recorder probe on macOS 26.6.2 (25G83).
 memcpy(&queue,(char*)impl+112,sizeof(queue));
 CFStringRef uid=NULL;UInt32 size=sizeof(uid);OSStatus status=AudioQueueGetProperty(queue,kAudioQueueProperty_CurrentDevice,&uid,&size);
 if(status) return status;
 UInt32 maximum=0;AudioQueueProcessingTapRef tap=NULL;
 status=AudioQueueProcessingTapNew(queue,old_callback,NULL,kAudioQueueProcessingTap_PreEffects,&maximum,&old_format,&tap);
 if(status) return status;
 if(old_format.mFormatID!=kAudioFormatLinearPCM || !(old_format.mFormatFlags&kAudioFormatFlagIsFloat) || old_format.mBitsPerChannel!=32) {AudioQueueProcessingTapDispose(tap);return -1;}
 fprintf(stderr,"AUDIT_RECORDER_TAP rate=%.0f channels=%u maxFrames=%u\n",old_format.mSampleRate,old_format.mChannelsPerFrame,maximum);
 return 0;
}
