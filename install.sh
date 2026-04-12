#!/bin/bash
# install.sh — 安装 Audio Input.app 并修复 TCC 麦克风权限
# 用法：把 .app 拖到和这个脚本同一目录，然后运行：bash install.sh

set -e

APP_NAME="Audio Input.app"
BUNDLE_ID="com.audioinput.app"
INSTALL_PATH="/Applications/$APP_NAME"

# 找 .app：先找同目录，再找 DMG 挂载根目录
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
if [ -d "$SCRIPT_DIR/$APP_NAME" ]; then
    SOURCE="$SCRIPT_DIR/$APP_NAME"
elif [ -d "/Volumes/Audio Input/$APP_NAME" ]; then
    SOURCE="/Volumes/Audio Input/$APP_NAME"
else
    echo "❌ 找不到 $APP_NAME，请把它和脚本放在同一目录"
    exit 1
fi

echo "→ 复制 $APP_NAME 到 /Applications ..."
sudo rm -rf "$INSTALL_PATH"
sudo cp -r "$SOURCE" "$INSTALL_PATH"
echo "  ✓ 安装完成"

# 获取新 binary 的 cdhash
BINARY="$INSTALL_PATH/Contents/MacOS/audio-input"
CDHASH=$(codesign -d -r- "$INSTALL_PATH" 2>&1 | grep 'cdhash' | sed 's/.*cdhash H"\([^"]*\)".*/\1/')
if [ -z "$CDHASH" ]; then
    echo "❌ 无法获取 cdhash，跳过 TCC 修复"
    exit 1
fi
echo "  cdhash: $CDHASH"

# 把 cdhash 转成 csreq blob 并更新 TCC
echo "→ 更新 TCC 麦克风权限 ..."
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
db_path = f"{__import__('os').path.expanduser('~')}/Library/Application Support/com.apple.TCC/TCC.db"
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
print("  ✓ TCC 权限已更新")
EOF

echo ""
echo "✅ 安装完成！打开 Audio Input，如需 Accessibility 权限请在系统设置里手动开启。"
