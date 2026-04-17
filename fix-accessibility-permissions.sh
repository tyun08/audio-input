#!/bin/bash
# fix-accessibility-permissions.sh — 修复 Audio Input 辅助功能（Accessibility）TCC 权限
#
# Accessibility 权限存储在系统级 TCC.db，受 SIP 保护。
# 本脚本尝试直接写入（SIP 关闭时有效），否则自动打开 System Settings。
#
# 用法：sudo bash fix-accessibility-permissions.sh

set -e

BUNDLE_ID="com.audioinput.app"
INSTALL_PATH="/Applications/Audio Input.app"

if [ "$(id -u)" -ne 0 ]; then
    echo "❌ 需要 sudo 权限，请使用：sudo bash $0"
    exit 1
fi

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
import sys, sqlite3, subprocess

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
db_path = "/Library/Application Support/com.apple.TCC/TCC.db"

try:
    db = sqlite3.connect(db_path)
    cur = db.execute(
        "SELECT COUNT(*) FROM access WHERE service='kTCCServiceAccessibility' AND client=?",
        (bundle_id,)
    )
    if cur.fetchone()[0] > 0:
        db.execute(
            "UPDATE access SET csreq=?, auth_value=2, auth_reason=3 WHERE service='kTCCServiceAccessibility' AND client=?",
            (csreq_blob, bundle_id)
        )
    else:
        db.execute(
            "INSERT INTO access (service,client,client_type,auth_value,auth_reason,auth_version,csreq,policy_id,indirect_object_identifier_type,indirect_object_identifier,indirect_object_code_identity,flags,last_modified,pid,pid_version,boot_uuid,last_reminded) VALUES ('kTCCServiceAccessibility',?,0,2,3,1,?,NULL,NULL,'UNUSED',NULL,0,strftime('%s','now'),NULL,NULL,'UNUSED',0)",
            (bundle_id, csreq_blob)
        )
    db.commit()
    db.close()
    print("✅ 辅助功能权限已修复，请重启 Audio Input app")
except sqlite3.OperationalError as e:
    if "readonly" in str(e):
        print("⚠️  SIP 已开启，无法直接写入系统 TCC.db")
        print("→  正在打开 System Settings → 辅助功能，请手动添加 Audio Input...")
        subprocess.run([
            'open',
            'x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility'
        ])
    else:
        print(f"❌ 数据库错误: {e}")
        sys.exit(1)
EOF
