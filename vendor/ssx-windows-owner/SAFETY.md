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

Explicit Cargo build-input digest reads permit a positive link count: Cargo may
hard-link its output and deps names on Windows. This narrow read role retains
non-reparse regular-file admission, the original opened object, byte/hash/size/
write-time checks, read-only sharing and a stable link count throughout hashing.
Staging copies those bytes into new private files; every installed package,
trusted image, lock and confidential-object role still requires exactly one
link. Build-input hashing never grants execution admission. The fixture-only
link-count reader exposes only finite metadata for the three explicit roles.

## Required actual qualification (not performed by this patch)

Pinned Windows x86_64 compile/link, strict lints/format, installed distribution without Cargo, console and redirected input bytes/EOF, actual overlapped IO completion, idle/backpressured input cancellation and dropped-close retry; DNS cancellation/deadline/protocol faults/descendants; atomic-job failure injection, unsupported/nested-job rejection and parent-death kill-on-close witnesses. Verify suspended child cannot execute before owner registration and inherited handles remain exactly bounded. Verify denied ACL/reparse/hash/rename controls and package lock contention. Windows 10+ client floor follows the retained Rust/MS source policy; an actual minimum-OS installation witness remains O04 work. Source AST parsing is not Windows compile/link/runtime proof.

Microsoft's UpdateProcThreadAttribute documentation explicitly lists JOB_LIST on Windows 10 / Windows Server 2016 onward: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute . This does not add Windows Server product scope. Exact CPython, AnyIO, toolchain and Windows API source pins are in the separately retained Windows source review packet.

## Client-platform acceptance extension

Token documents use the existing trusted owner set but a separate confidentiality
policy: public generic/read/execute grants are refused as well as untrusted writes.
Package integrity continues to permit public readers. Private creation supplies a
protected DACL at CreateFile time. Confidential parent guards permit read/write
sharing because Windows opens the rename destination directory with FILE_ADD_FILE;
they still deny delete sharing, retaining the parent replacement fence through
same-parent atomic publication. Parent ACL, reparse, link and identity admission
is unchanged. Token read handles permit delete sharing for replacement but deny
writers; package/image handles continue to deny write/delete sharing. Identity
and link-count checks span the read.
Read admission does not create directories or renew any shared state. A failed
create-new collision never transfers cleanup ownership of the existing path.

The nondefault `fixtures` feature additionally permits SetFileSecurityW solely to
construct adversarial ACLs, and OpenProcess/GetProcessTimes solely to retain a
read-only process witness with an exact creation-time check. No termination right
or Job handle is returned to that witness. The required-feature gateway probe
owns its real NativeChild/Job, publishes only PID plus original creation time
after helper protocol entry, and is terminated through its original Child.
The wrong-Job control passes a live pipe handle in JOB_LIST; normal code has no
fault parameter. Even unexpected kernel acceptance remains suspended and is
terminated through the original returned process handle before failure. This
fixture-only TerminateProcess path is never selected by a production build.

These are source-level claims until actual native Windows/MSVC compilation and
runtime receipts exist. The new five-platform workflow is a candidate witness;
no Windows minimum-version, release signature or production routing claim follows.
