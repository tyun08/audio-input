// Entry point: collect all tests and run them. Exits non-zero on any
// failure so CI / a shell script can tell pass from fail.

print("ImkHelper tests")
print("===============")
print("")
print("ProtocolTests")
print("-------------")
let all = protocolTests + socketTests
print("")
print("SocketServerTests (interleaved with protocol tests above for readability)")
runAll(all)
