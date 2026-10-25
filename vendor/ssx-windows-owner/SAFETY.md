# Private Windows ownership boundary: review contract

This excluded private dependency is the sole new unsafe boundary. Application, MCP and workspace unsafe_code=forbid remain unchanged. It is not a general command runner and exports no raw HANDLE or pointer constructor. Safe callers select only a digest-pinned installed DNS or input helper; input takes an owned duplicate of stdin. The protocol and shared resolver admission/JoinSet/close state machine remain the existing ones.

## Finite foreign calls

Process and IO: CreateJobObjectW, SetInformationJobObject, QueryInformationJobObject, TerminateJobObject, InitializeProcThreadAttributeList, UpdateProcThreadAttribute, DeleteProcThreadAttributeList, CreateProcessW, ResumeThread, WaitForSingleObject, GetExitCodeProcess, CreateFileW, CancelIoEx. Tokio's existing create_with_security_attributes_raw is the only unsafe pipe construction wrapper.

Trust and allocation: GetCurrentProcess, DuplicateHandle, GetStdHandle, OpenProcessToken, GetTokenInformation, ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, GetSecurityInfo, IsValidSid, EqualSid, IsWellKnownSid, IsValidAcl, GetAce, GetFileInformationByHandle, CreateDirectoryW, LocalFree. Standard OwnedHandle owns newly created/duplicated handles and closes them; existing borrowed stdin/current-process handles are never adopted. LocalFree allocations and addrinfo results have distinct exactly-once guards.

DNS: WSAStartup, WSACleanup, getaddrinfo, freeaddrinfo. A successful private startup reference is balanced after its result list is freed. There is no extra UDP bind. Decimal service text is preserved through native GAI; only the actual numeric async-connect boundary uses the source's unsigned-short mask. Canonical validation does not substitute a narrowed GAI service string.

## Memory and lifetime invariants

All pointer arguments refer to live, aligned, initialized storage, NUL-terminated owned strings, or borrowed OS result spans. Security descriptors and token-user buffers outlive all SID/ACL borrows. Result-list family and sockaddr length are checked before unaligned reads; no result pointer escapes. Attribute storage, inherited handle list, job list, mutable command line and STARTUPINFOEX storage remain unchanged through CreateProcessW. The successful process/thread handles are adopted exactly once. Unsafe union reads are restricted to the sockaddr family established by native GAI.

## Process and IO invariants

CREATE_SUSPENDED plus atomic PROC_THREAD_ATTRIBUTE_JOB_LIST assigns the child to the private kill-on-close job at CreateProcessW. HANDLE_LIST permits only the three explicit stdin/stdout/stderr child endpoints. There is no post-spawn AssignProcessToJobObject gap or PID signal authority. Unsupported job attributes/nested jobs fail before user code runs. Registration precedes ResumeThread. Original process handles prove process exit; job accounting proves no surviving descendants. Cancellation alone, Drop, CancelIoEx and elapsed deadlines are not retirement proof. Started IO remains in owned tasks; timeouts preserve a residual original child/IO owner and return failure. No parent runtime task performs blocking stdin/GAI or uses spawn_blocking for these operations.

Installed paths are fixed roles in a three-artifact record. Reparse points, wrong types, unexpected link counts, digest/identity changes, untrusted write/delete/ownership DACLs, noncanonical records and guessed helper paths fail closed. Trusted executable handles deny replacement/write sharing through launch. Current user, SYSTEM and Administrators are the explicit trusted owner/writer principals. This is local package custody, not protection from a malicious trusted principal.

## Required actual qualification (not performed by this patch)

Pinned Windows x86_64 compile/link, strict lints/format, installed distribution without Cargo, console and redirected input bytes/EOF, actual overlapped IO completion, idle/backpressured input cancellation and dropped-close retry; DNS cancellation/deadline/protocol faults/descendants; atomic-job failure injection, unsupported/nested-job rejection and parent-death kill-on-close witnesses. Verify suspended child cannot execute before owner registration and inherited handles remain exactly bounded. Verify denied ACL/reparse/hash/rename controls and package lock contention. Windows 10+ client floor follows the retained Rust/MS source policy; an actual minimum-OS installation witness remains O04 work. Source AST parsing is not Windows compile/link/runtime proof.

Microsoft's UpdateProcThreadAttribute documentation explicitly lists JOB_LIST on Windows 10 / Windows Server 2016 onward: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute . This does not add Windows Server product scope. Exact CPython, AnyIO, toolchain and Windows API source pins are in the separately retained Windows source review packet.
