/* Temporary offline source-version comparison harness; no runtime path. */
#include <libxml/HTMLparser.h>
#include <libxml/HTMLtree.h>
#include <libxml/parser.h>
#include <libxml/xmlmemory.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
static xmlParserErrors refuse(void *a,const char*b,const char*c,xmlResourceType d,xmlParserInputFlags e,xmlParserInput **out){(void)a;(void)b;(void)c;(void)d;(void)e;if(out)*out=NULL;return XML_IO_LOAD_ERROR;}
static void hex(const xmlChar *s){if(!s)return;while(*s){printf("%02x",(unsigned int)*s);s++;}}
static void walk(xmlDocPtr doc,xmlNodePtr node,unsigned int depth){for(;node;node=node->next){if(node->type==XML_ELEMENT_NODE){printf("E%u:",depth);hex(node->name);for(xmlAttrPtr a=node->properties;a;a=a->next){putchar(':');hex(a->name);putchar('=');xmlChar *v=xmlNodeListGetString(doc,a->children,1);hex(v);xmlFree(v);}putchar('\n');walk(doc,node->children,depth+1);}else if(node->type==XML_TEXT_NODE||node->type==XML_CDATA_SECTION_NODE){printf("T%u:",depth);hex(node->content);putchar('\n');}}}
int main(void){
 unsigned char head[8];if(fread(head,1,8,stdin)!=8)return 2;uint64_t count=0;for(int i=0;i<8;i++)count=(count<<8)|head[i];if(count>33554432)return 3;
 uint32_t *points=calloc(count?count:1,sizeof(uint32_t));if(!points)return 4;uint32_t maximum=0;
 for(uint64_t i=0;i<count;i++){unsigned char p[4];if(fread(p,1,4,stdin)!=4)return 5;points[i]=((uint32_t)p[0]<<24)|((uint32_t)p[1]<<16)|((uint32_t)p[2]<<8)|p[3];if(points[i]>0x10ffff)return 6;if(points[i]>maximum)maximum=points[i];}
 size_t width=maximum<=255?1:maximum<=65535?2:4;unsigned char *data=calloc((size_t)count*width+4,1);if(!data)return 7;
 uint16_t endian=1;int little=*(unsigned char*)&endian;
 for(uint64_t i=0;i<count;i++){if(width==1)data[i]=(unsigned char)points[i];else for(size_t j=0;j<width;j++)data[i*width+j]=(unsigned char)(points[i]>>(8*(little?j:width-j-1)));}
 const char *encoding=maximum<=127?"UTF-8":maximum<=255?"ISO-8859-1":width==2?(little?"UTF-16LE":"UTF-16BE"):(little?"UTF-32LE":"UTF-32BE");
 htmlParserCtxtPtr context=htmlNewParserCtxt();xmlCtxtSetResourceLoader(context,refuse,NULL);
 htmlDocPtr document=htmlCtxtReadMemory(context,(char*)data,(int)(count*width),NULL,encoding,HTML_PARSE_RECOVER|HTML_PARSE_NONET|HTML_PARSE_COMPACT);
 if(document){printf("PARSED\n");if(xmlDocGetRootElement(document))walk(document,xmlDocGetRootElement(document),0);else printf("NO_ROOT\n");}else printf("NO_DOCUMENT\n");
 if(document)xmlFreeDoc(document);htmlFreeParserCtxt(context);free(data);free(points);return 0;
}
