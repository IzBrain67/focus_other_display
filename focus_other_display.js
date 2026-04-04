#!/usr/bin/osascript -l JavaScript

// ============================================================
// focus_other_display.js (v3)
// 反対側のディスプレイの最前面ウィンドウにフォーカスする
//
// v3改善: activate()ではなく AXRaise で特定ウィンドウだけを前面化
//   → 同じアプリが両ディスプレイにある場合も正しく動作
//
// 使い方:
//   osascript -l JavaScript focus_other_display.js
// ============================================================

ObjC.import("CoreGraphics");
ObjC.import("AppKit");

// デバッグ出力: true=有効, false=無効
var DEBUG = false;

function debug(msg) {
    if (!DEBUG) return;
    $.NSFileHandle.fileHandleWithStandardError
        .writeData($.NSString.alloc.initWithString("[DEBUG] " + msg + "\n")
        .dataUsingEncoding($.NSUTF8StringEncoding));
}

function run() {
    debug("=== スクリプト開始 (v3) ===");

    // --- NSScreen でディスプレイ情報を取得 ---
    var screens = $.NSScreen.screens;
    var count = screens.count;
    debug("検出ディスプレイ数: " + count);

    if (count < 2) {
        debug("ERROR: ディスプレイが1枚しかありません");
        return "ERROR: ディスプレイが1枚しか検出されません";
    }

    var displays = [];
    for (var i = 0; i < count; i++) {
        var screen = screens.objectAtIndex(i);
        var frame = screen.frame;
        displays.push({
            x: frame.origin.x,
            y: frame.origin.y,
            w: frame.size.width,
            h: frame.size.height
        });
    }
    displays.sort(function(a, b) { return a.x - b.x; });

    for (var i = 0; i < displays.length; i++) {
        var d = displays[i];
        debug("ディスプレイ[" + i + "]: origin=(" + d.x + "," + d.y + ") size=" + d.w + "x" + d.h);
    }

    // --- 現在フォーカス中のアプリとウィンドウ ---
    var sysEvents = Application("System Events");
    var frontProc = sysEvents.processes.whose({frontmost: true})[0];
    var frontAppName = frontProc.name();
    debug("現在フォーカス中: " + frontAppName);

    var currentDisplayIdx = 0;
    try {
        var fw = frontProc.windows[0];
        var fwPos = fw.position();
        var fwSize = fw.size();
        var fwCenterX = fwPos[0] + fwSize[0] / 2;
        debug("最前面ウィンドウ: pos=(" + fwPos[0] + "," + fwPos[1] + ") size=" + fwSize[0] + "x" + fwSize[1] + " centerX=" + fwCenterX);

        for (var i = 0; i < displays.length; i++) {
            var d = displays[i];
            if (fwCenterX >= d.x && fwCenterX < d.x + d.w) {
                currentDisplayIdx = i;
                break;
            }
        }
    } catch (e) {
        debug("WARNING: ウィンドウ位置取得失敗: " + e.message);
    }
    debug("現在のディスプレイ: [" + currentDisplayIdx + "]");

    // --- 反対側のディスプレイ ---
    var targetDisplayIdx = (currentDisplayIdx === 0) ? 1 : 0;
    var targetDisplay = displays[targetDisplayIdx];
    debug("ターゲットディスプレイ: [" + targetDisplayIdx + "] origin=(" + targetDisplay.x + "," + targetDisplay.y + ")");

    // --- 全アプリのウィンドウを走査し、ターゲットディスプレイ上のものを探す ---
    debug("--- ウィンドウ走査開始 ---");
    var allProcs = sysEvents.processes.whose({backgroundOnly: false})();

    var targetProc = null;
    var targetWinRef = null;
    var targetProcName = null;
    var targetWinTitle = null;

    for (var p = 0; p < allProcs.length; p++) {
        var proc = allProcs[p];
        var procName = "";
        try { procName = proc.name(); } catch(e) { continue; }

        var windows;
        try { windows = proc.windows(); } catch(e) { continue; }

        for (var w = 0; w < windows.length; w++) {
            var win = windows[w];
            var title, pos, size;
            try {
                title = win.name() || "(無題)";
                pos = win.position();
                size = win.size();
            } catch(e) { continue; }

            var wX = pos[0], wY = pos[1];
            var wW = size[0], wH = size[1];

            if (wW < 50 || wH < 50) continue;

            var wCenterX = wX + wW / 2;
            var onTarget = (wCenterX >= targetDisplay.x && wCenterX < targetDisplay.x + targetDisplay.w);

            // 現在フォーカス中のアプリの、現在のディスプレイ上のウィンドウはスキップ
            // ただし、同じアプリでもターゲットディスプレイ上のウィンドウは候補にする
            var isCurrentAppOnCurrentDisplay = (procName === frontAppName && !onTarget);
            if (procName === frontAppName && !onTarget) {
                debug("  SKIP(現在のアプリ/別ディスプレイ): " + procName + " - " + title);
                continue;
            }

            debug("  " + procName + " - " + title + " pos=(" + wX + "," + wY + ") centerX=" + Math.round(wCenterX) + " onTarget=" + onTarget);

            if (onTarget && !targetWinRef) {
                targetProc = proc;
                targetWinRef = win;
                targetProcName = procName;
                targetWinTitle = title;
                debug("  >>> ターゲットとして選択！");
            }
        }
    }

    if (!targetWinRef) {
        debug("ERROR: 反対側のディスプレイにウィンドウが見つかりません");
        return "ERROR: 反対側のディスプレイにウィンドウが見つかりません";
    }

    // --- マウスをターゲットウィンドウの中央へ移動 ---
    // NSScreenの座標系は信頼できないため、
    // ターゲットウィンドウの実座標（System Events）から中央を算出する
    var twPos, twSize;
    try {
        twPos = targetWinRef.position();
        twSize = targetWinRef.size();
    } catch(e) {
        debug("WARNING: ターゲットウィンドウの座標取得失敗: " + e.message);
        twPos = [0, 0];
        twSize = [targetDisplay.w, targetDisplay.h];
    }
    var centerX = twPos[0] + twSize[0] / 2;
    var centerY = twPos[1] + twSize[1] / 2;
    debug("マウス移動先: (" + Math.round(centerX) + "," + Math.round(centerY) + ") [ウィンドウ pos=(" + twPos[0] + "," + twPos[1] + ") size=" + twSize[0] + "x" + twSize[1] + "]");

    try {
        var moveEvent = $.CGEventCreateMouseEvent(
            null,
            $.kCGEventMouseMoved,
            $.CGPointMake(centerX, centerY),
            $.kCGMouseButtonLeft
        );
        $.CGEventPost($.kCGHIDEventTap, moveEvent);
        debug("マウス移動完了");
    } catch(e) {
        debug("WARNING: マウス移動失敗: " + e.message);
    }

    // --- 特定のウィンドウだけを前面化 ---
    debug("AXRaise実行: " + targetProcName + " [" + targetWinTitle + "]");

    // 1) 対象ウィンドウを AXRaise で最前面にする
    try {
        var actions = targetWinRef.actions();
        debug("利用可能なアクション: " + actions.map(function(a) { return a.name(); }).join(", "));
        targetWinRef.actions["AXRaise"].perform();
        debug("AXRaise 成功");
    } catch(e) {
        debug("WARNING: AXRaise失敗: " + e.message + " → フォールバックでactivate使用");
    }

    // 2) アプリ自体もアクティブにする（キーボードフォーカスを移すため）
    //    activate()は全ウィンドウを前面にするが、
    //    直前のAXRaiseで対象ウィンドウが最前面になっているのでOK
    try {
        targetProc.frontmost = true;
        debug("frontmost = true 設定完了");
    } catch(e) {
        debug("WARNING: frontmost設定失敗: " + e.message);
        // フォールバック
        var app = Application(targetProcName);
        app.activate();
        debug("activate() フォールバック完了");
    }

    var srcName = (currentDisplayIdx === 0) ? "左" : "右";
    var dstName = (targetDisplayIdx === 0) ? "左" : "右";
    var result = "OK: " + srcName + " [" + frontAppName + "] → " + dstName + " [" + targetProcName + " - " + targetWinTitle + "]";
    debug(result);
    debug("=== スクリプト終了 ===");
    return result;
}
