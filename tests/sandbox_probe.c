#include <stdio.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <sys/socket.h>
#include <sys/wait.h>
#include <arpa/inet.h>
#include <pthread.h>
#include <stdlib.h>
#include <signal.h>
static void *thread(void *x){return x;}
int main(int argc,char **argv){
 if(argc!=5)return 2;
 int failed=0;
 int fd=open(argv[1],O_RDONLY);if(fd<0)failed++;else close(fd);
 fd=open(argv[2],O_RDONLY);if(fd>=0){failed++;close(fd);}
 fd=open(argv[3],O_WRONLY|O_CREAT,0600);if(fd<0)failed++;else {if(write(fd,"ok",2)!=2)failed++;close(fd);}
 if(truncate(argv[2],0)==0)failed++;
 fd=open(argv[1],O_RDONLY|O_TRUNC);if(fd>=0){failed++;close(fd);}
 pid_t p=fork();if(p==0)_exit(0);if(p>=0){failed++;waitpid(p,NULL,0);}
 pthread_t t;if(pthread_create(&t,NULL,thread,NULL)!=0)failed++;else pthread_join(t,NULL);
 int s=socket(AF_INET,SOCK_STREAM,0);struct sockaddr_in a={.sin_family=AF_INET,.sin_port=htons(4173)};inet_pton(AF_INET,"127.0.0.1",&a.sin_addr);
 int connected=connect(s,(void*)&a,sizeof(a))==0;close(s);
 if(connected!=atoi(argv[4]))failed++;
 if(getenv("DELTA_TOKEN")!=NULL)failed++;
 if(kill(getppid(),0)==0)failed++;
 printf("{\"failed\":%d,\"read_allowed\":true,\"secret_read_blocked\":true,\"write_allowed\":true,\"truncate_blocked\":true,\"fork_blocked\":true,\"threads_allowed\":true,\"outbound_connected\":%s,\"token_environment_scrubbed\":true}\n",failed,connected?"true":"false");
 return failed?1:0;
}
