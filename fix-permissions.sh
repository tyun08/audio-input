#!/bin/bash
# fix-permissions.sh — 为当前用户修复 Audio Input 麦克风 TCC 权限
# 不需要 sudo，每个用户自己运行一次即可

set -e

BUNDLE_ID="com.audioinput.app"
INSTALL_PATH="/Applications/Audio Input.app"

if [ ! -d "$INSTALL_PATH" ]; then
    echo "❌ 找不到 $INSTALL_PATH，请先安装 app"
    exit 1
fi

CDHASH=$(codesign -d -r- "$INSTALL_PATH" 2>&1 | grep 'cdhash' | sed 's/.*cdhash H"\([^"]*\)".*/\1/')
if [ -z "$CDHASH" ]; then
    echo "❌ 无法获取 cdhash"
    exit 1
fi

python3 - "$CDHASH" "$BUNDLE_ID" << 'EOF'
import sys, sqlite3, subprocess, os

cdhash, bundle_id = sys.argv[1], sys.argv[2]

result = subprocess.run(
    ['csreq', '-r', '-', '-b', '/dev/stdout'],
    input=f'cdhash H"{cdhash}"'.encode(),
    capture_output=True
)
if result.returncode != 0:
    print("❌ csreq 转换失败:", result.stderr.decode())
    sys.exit(1)

csreq_blob = result.stdout
db_path = os.path.expanduser("~/Library/Application Support/com.apple.TCC/TCC.db")
db = sqlite3.connect(db_path)
cur = db.execute(
    "SELECT COUNT(*) FROM access WHERE service='kTCCServiceMicrophone' AND client=?",
    (bundle_id,)
)
if cur.fetchone()[0] > 0:
    db.execute(
        "UPDATE access SET csreq=?, auth_value=2, auth_reason=3 WHERE service='kTCCServiceMicrophone' AND client=?",
        (csreq_blob, bundle_id)
    )
else:
    db.execute(
        "INSERT INTO access (service,client,client_type,auth_value,auth_reason,auth_version,csreq,policy_id,indirect_object_identifier_type,indirect_object_identifier,indirect_object_code_identity,flags,last_modified,pid,pid_version,boot_uuid,last_reminded) VALUES ('kTCCServiceMicrophone',?,0,2,3,1,?,NULL,NULL,'UNUSED',NULL,0,strftime('%s','now'),NULL,NULL,'UNUSED',0)",
        (bundle_id, csreq_blob)
    )
db.commit()
db.close()
print(f"✅ 麦克风权限已为当前用户修复")
EOF
