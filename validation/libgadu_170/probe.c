/* SPDX-License-Identifier: MIT
 * SPDX-FileCopyrightText: 2026 webs1821
 * Standalone API client for external, unmodified libgadu 1.7.0.
 * Synthetic accounts only. No rGG code is linked into this process.
 */
#include <arpa/inet.h>
#include <errno.h>
#include <libgadu.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/select.h>
#include <unistd.h>

static void require(int condition, const char *what) {
    if (!condition) { fprintf(stderr, "probe failed: %s (errno=%d)\n", what, errno); exit(1); }
}
static void hex(const unsigned char *text) {
    if (text) for (size_t i = 0; text[i]; ++i) printf("%02x", text[i]);
}
static int digit(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    return -1;
}
static void decode(const char *text, unsigned char *out) {
    size_t size = strlen(text);
    require(size <= 4096 && size % 2 == 0, "hex length");
    for (size_t i = 0; i < size; i += 2) {
        int high = digit(text[i]), low = digit(text[i + 1]);
        require(high >= 0 && low >= 0 && (high || low), "hex byte/nonzero");
        out[i / 2] = (unsigned char)(high * 16 + low);
    }
    out[size / 2] = 0;
}
static void event(const struct gg_event *e) {
    switch (e->type) {
    case GG_EVENT_NONE: break;
    case GG_EVENT_PONG: puts("{\"event\":\"pong\"}"); break;
    case GG_EVENT_ACK:
        printf("{\"event\":\"ack\",\"recipient\":%u,\"status\":%d,\"seq\":%d}\n",
               e->event.ack.recipient, e->event.ack.status, e->event.ack.seq); break;
    case GG_EVENT_MSG:
        printf("{\"event\":\"message\",\"sender\":%u,\"class\":%d,\"time\":%lld,\"text\":\"",
               e->event.msg.sender, e->event.msg.msgclass, (long long)e->event.msg.time);
        hex(e->event.msg.message); puts("\"}"); break;
    case GG_EVENT_STATUS60:
        printf("{\"event\":\"status\",\"uin\":%u,\"status\":%d,\"description\":\"",
               e->event.status60.uin, e->event.status60.status);
        hex((const unsigned char *)e->event.status60.descr); puts("\"}"); break;
    case GG_EVENT_NOTIFY60:
        for (size_t i = 0; e->event.notify60[i].uin; ++i) {
            require(i < 4000, "notify bound");
            printf("{\"event\":\"notify\",\"uin\":%u,\"status\":%d,\"description\":\"",
                   e->event.notify60[i].uin, e->event.notify60[i].status);
            hex((const unsigned char *)e->event.notify60[i].descr); puts("\"}");
        }
        break;
    case GG_EVENT_DISCONNECT: puts("{\"event\":\"disconnect\"}"); break;
    case GG_EVENT_USERLIST:
        printf("{\"event\":\"userlist\",\"type\":%d,\"text\":\"", e->event.userlist.type);
        hex((const unsigned char *)e->event.userlist.reply); puts("\"}"); break;
    case GG_EVENT_PUBDIR50_READ:
    case GG_EVENT_PUBDIR50_WRITE:
    case GG_EVENT_PUBDIR50_SEARCH_REPLY: {
        gg_pubdir50_t result = e->event.pubdir50;
        int count = gg_pubdir50_count(result); require(count >= 0 && count <= 20, "directory count");
        printf("{\"event\":\"directory\",\"type\":%d,\"seq\":%u,\"count\":%d,\"next\":%u,\"rows\":[",
               gg_pubdir50_type(result), gg_pubdir50_seq(result), count, gg_pubdir50_next(result));
        const char *fields[] = {GG_PUBDIR50_UIN, GG_PUBDIR50_NICKNAME, GG_PUBDIR50_FIRSTNAME,
                                GG_PUBDIR50_CITY, GG_PUBDIR50_BIRTHYEAR, GG_PUBDIR50_STATUS};
        for (int row = 0; row < count; ++row) {
            if (row) putchar(',');
            putchar('{');
            for (size_t field = 0; field < sizeof(fields) / sizeof(fields[0]); ++field) {
                if (field) putchar(',');
                printf("\"%s\":\"", fields[field]);
                hex((const unsigned char *)gg_pubdir50_get(result, row, fields[field])); putchar('"');
            }
            putchar('}');
        }
        puts("]}"); break;
    }
    default: printf("{\"event\":\"other\",\"type\":%d}\n", e->type); break;
    }
}

static void directory(struct gg_session *session, const char *command, unsigned seq, const char *argument) {
    require(seq > 0 && seq < 10000, "directory sequence");
    int type = !strcmp(command, "profile") || !strcmp(command, "unpublish") ? GG_PUBDIR50_WRITE :
               !strcmp(command, "read") ? GG_PUBDIR50_READ : GG_PUBDIR50_SEARCH;
    gg_pubdir50_t request = gg_pubdir50_new(type); require(request != NULL, "directory allocation");
    require(gg_pubdir50_seq_set(request, seq) == 0, "directory sequence");
    if (!strcmp(command, "profile") || !strcmp(command, "search")) {
        unsigned char nickname[2049]; decode(argument, nickname);
        require(gg_pubdir50_add(request, GG_PUBDIR50_NICKNAME, (const char *)nickname) == 0, "nickname");
    }
    if (!strcmp(command, "profile")) {
        require(gg_pubdir50_add(request, GG_PUBDIR50_FIRSTNAME, "Jan") == 0, "first name");
        require(gg_pubdir50_add(request, GG_PUBDIR50_CITY, "\xa3\xf3\x64\x9f") == 0, "city");
        require(gg_pubdir50_add(request, GG_PUBDIR50_BIRTHYEAR, "1987") == 0, "birth year");
    }
    if (!strcmp(command, "age")) require(gg_pubdir50_add(request, GG_PUBDIR50_BIRTHYEAR, "1980 1990") == 0, "age");
    if (!strcmp(command, "active")) require(gg_pubdir50_add(request, GG_PUBDIR50_ACTIVE, "1") == 0, "active");
    require(gg_pubdir50(session, request) != 0, "directory send");
    gg_pubdir50_free(request);
}
int main(int argc, char **argv) {
    require(argc == 3 || (argc == 4 && (!strcmp(argv[3], "wrong-password") || !strcmp(argv[3], "description"))), "arguments");
    int wrong = argc == 4 && !strcmp(argv[3], "wrong-password");
    int described = argc == 4 && !strcmp(argv[3], "description");
    char *end;
    long port = strtol(argv[1], &end, 10); require(!*end && port > 0 && port <= 65535, "port");
    long uin = strtol(argv[2], &end, 10); require(!*end && uin >= 1 && uin <= 3, "synthetic UIN");
    alarm(90); signal(SIGPIPE, SIG_IGN); setvbuf(stdout, NULL, _IONBF, 0);
    /* No DNS/discovery/TLS/proxy/DCC; the driver also isolates networking. */
    gg_proxy_enabled = 0; gg_dcc_ip = inet_addr("127.0.0.1"); gg_dcc_port = 0;
    struct gg_login_params login = {0};
    login.uin = (uin_t)uin; login.password = wrong ? "Incorrect9!" : "LocalTest9!";
    login.status = described ? GG_STATUS_AVAIL_DESCR : GG_STATUS_AVAIL;
    login.status_descr = described ? "\xaf\xf3\xb3\xe6" : NULL;
    login.protocol_version = 0x20;
    login.server_addr = inet_addr("127.0.0.1"); login.server_port = (uint16_t)port;
    struct gg_session *session = gg_login(&login);
    if (wrong) {
        require(session == NULL && errno == EACCES, "bad password rejected");
        puts("{\"event\":\"login_rejected\"}"); return 0;
    }
    require(session != NULL, "login");
    printf("{\"event\":\"login\",\"uin\":%u,\"protocol_version\":%d}\n", session->uin, session->protocol_version);
    char line[4200], command[32], argument[4100]; unsigned target;
    for (;;) {
        fd_set reads; FD_ZERO(&reads); FD_SET(STDIN_FILENO, &reads); FD_SET(session->fd, &reads);
        struct timeval timeout = {5, 0};
        int count = select(session->fd + 1, &reads, NULL, NULL, &timeout);
        require(count >= 0 || errno == EINTR, "select");
        if (count <= 0) continue;
        if (FD_ISSET(STDIN_FILENO, &reads)) {
            if (!fgets(line, sizeof(line), stdin)) break;
            require(strchr(line, '\n') != NULL, "command length");
            argument[0] = 0; target = 0;
            int fields = sscanf(line, "%31s %u %4099s", command, &target, argument);
            require(fields >= 1, "command");
            if (!strcmp(command, "quit")) break;
            if (!strcmp(command, "empty")) require(gg_notify(session, NULL, 0) == 0, "empty notify");
            else if (!strcmp(command, "notify") && fields == 2 && target >= 1 && target <= 3) {
                uin_t friend = target; require(gg_notify(session, &friend, 1) == 0, "notify");
            } else if (!strcmp(command, "ping")) require(gg_ping(session) == 0, "ping");
            else if (!strcmp(command, "send") && fields == 3 && target >= 1 && target <= 3) {
                unsigned char text[2049]; decode(argument, text);
                int seq = gg_send_message(session, GG_CLASS_CHAT, target, text);
                require(seq >= 0, "send message"); printf("{\"event\":\"sent\",\"seq\":%d}\n", seq);
            } else if (!strcmp(command, "status") && fields == 3) {
                unsigned char text[2049]; decode(argument, text);
                require(target == GG_STATUS_AVAIL_DESCR || target == GG_STATUS_INVISIBLE, "status allowlist");
                require(gg_change_status_descr(session, (int)target, (const char *)text) == 0, "status");
            } else if ((!strcmp(command, "profile") || !strcmp(command, "search")) && fields == 3) {
                directory(session, command, target, argument);
            } else if ((!strcmp(command, "read") || !strcmp(command, "age") || !strcmp(command, "active") || !strcmp(command, "unpublish")) && fields == 2) {
                directory(session, command, target, "");
            } else if (!strcmp(command, "get") && fields == 1) {
                require(gg_userlist_request(session, GG_USERLIST_GET, NULL) == 0, "get contacts");
            } else if (!strcmp(command, "put") && fields == 3) {
                unsigned char text[2049]; decode(argument, text);
                require(gg_userlist_request(session, GG_USERLIST_PUT, (const char *)text) == 0, "put contacts");
            } else if (!strcmp(command, "clear-list") && fields == 1) {
                require(gg_userlist_request(session, GG_USERLIST_PUT, "") == 0, "clear contacts");
            } else require(0, "unknown command");
            printf("{\"event\":\"command\",\"name\":\"%s\"}\n", command);
        }
        if (FD_ISSET(session->fd, &reads)) {
            struct gg_event *e = gg_watch_fd(session);
            if (!e) { puts("{\"event\":\"closed\"}"); break; }
            event(e); gg_event_free(e);
        }
    }
    gg_logoff(session); gg_free_session(session); return 0;
}
