/**
 * 遮罩点击关闭：仅当 mousedown 与 click 都落在遮罩自身时关闭，
 * 避免对话框内拖选文字后鼠标在遮罩上松开误关。
 * 代码路径: kk_novel_ai/src/utils/backdropDismiss.js
 *
 * @param {() => void} onDismiss
 */
export function createBackdropDismiss(onDismiss) {
  let downOnSelf = false;
  return {
    onMouseDown(e) {
      downOnSelf = e.target === e.currentTarget;
    },
    onClick(e) {
      if (e.target === e.currentTarget && downOnSelf) {
        onDismiss();
      }
      downOnSelf = false;
    },
  };
}
