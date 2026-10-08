// 이미지 비교 로직
// Canvas API를 사용하여 두 이미지를 비교하고 델타를 계산합니다.

class ImageComparator {
  constructor() {
    this.cache = new Map();
  }

  /**
   * 이미지를 로드합니다.
   * @param {string} src - 이미지 경로
   * @returns {Promise<HTMLImageElement>}
   */
  async loadImage(src) {
    if (this.cache.has(src)) {
      return this.cache.get(src);
    }

    return new Promise((resolve, reject) => {
      const img = new Image();
      img.crossOrigin = 'anonymous';
      img.onload = () => {
        this.cache.set(src, img);
        resolve(img);
      };
      img.onerror = () => reject(new Error(`이미지 로드 실패: ${src}`));
      img.src = src;
    });
  }

  /**
   * 이미지를 Canvas에 그립니다.
   * @param {HTMLImageElement} img
   * @returns {HTMLCanvasElement}
   */
  imageToCanvas(img) {
    const canvas = document.createElement('canvas');
    canvas.width = img.naturalWidth;
    canvas.height = img.naturalHeight;
    const ctx = canvas.getContext('2d');
    ctx.drawImage(img, 0, 0);
    return canvas;
  }

  /**
   * 두 이미지를 비교하여 델타를 계산합니다.
   * @param {HTMLImageElement} img1 - 레거시 이미지
   * @param {HTMLImageElement} img2 - 현재 이미지
   * @returns {Object} 비교 결과
   */
  compare(img1, img2) {
    const w = Math.min(img1.naturalWidth, img2.naturalWidth);
    const h = Math.min(img1.naturalHeight, img2.naturalHeight);

    const canvas1 = this.imageToCanvas(img1);
    const canvas2 = this.imageToCanvas(img2);

    const ctx1 = canvas1.getContext('2d');
    const ctx2 = canvas2.getContext('2d');

    const data1 = ctx1.getImageData(0, 0, w, h).data;
    const data2 = ctx2.getImageData(0, 0, w, h).data;

    let matchingPixels = 0;
    let totalPixels = w * h;
    let maxDelta = 0;
    let diffPixels = 0;

    // 델타 이미지 데이터
    const deltaData = new Uint8ClampedArray(w * h * 4);

    for (let i = 0; i < data1.length; i += 4) {
      const r1 = data1[i], g1 = data1[i + 1], b1 = data1[i + 2], a1 = data1[i + 3];
      const r2 = data2[i], g2 = data2[i + 1], b2 = data2[i + 2], a2 = data2[i + 3];

      const dr = Math.abs(r1 - r2);
      const dg = Math.abs(g1 - g2);
      const db = Math.abs(b1 - b2);
      const da = Math.abs(a1 - a2);

      const pixelDelta = Math.max(dr, dg, db, da);

      if (pixelDelta > maxDelta) {
        maxDelta = pixelDelta;
      }

      if (pixelDelta === 0) {
        matchingPixels++;
        // 일치하는 픽셀: 반투명 회색
        deltaData[i] = 128;
        deltaData[i + 1] = 128;
        deltaData[i + 2] = 128;
        deltaData[i + 3] = 30;
      } else {
        diffPixels++;
        // 다른 픽셀: 빨간색으로 표시 (강도에 따라)
        const intensity = Math.min(255, pixelDelta * 2);
        deltaData[i] = 255;
        deltaData[i + 1] = 0;
        deltaData[i + 2] = 0;
        deltaData[i + 3] = intensity;
      }
    }

    const matchFraction = matchingPixels / totalPixels;

    return {
      matchFraction,
      matchingPixels,
      totalPixels,
      diffPixels,
      maxDelta,
      deltaData,
      width: w,
      height: h
    };
  }

  /**
   * 델타 이미지를 Canvas로 생성합니다.
   * @param {Object} result - compare() 결과
   * @returns {HTMLCanvasElement}
   */
  createDeltaCanvas(result) {
    const canvas = document.createElement('canvas');
    canvas.width = result.width;
    canvas.height = result.height;
    const ctx = canvas.getContext('2d');
    const imageData = new ImageData(result.deltaData, result.width, result.height);
    ctx.putImageData(imageData, 0, 0);
    return canvas;
  }

  /**
   * 두 이미지를 나란히 비교합니다.
   * @param {string} legacySrc - 레거시 이미지 경로
   * @param {string} currentSrc - 현재 이미지 경로
   * @returns {Promise<Object>} 비교 결과
   */
  async compareImages(legacySrc, currentSrc) {
    const [legacyImg, currentImg] = await Promise.all([
      this.loadImage(legacySrc),
      this.loadImage(currentSrc)
    ]);

    const result = this.compare(legacyImg, currentImg);
    result.legacyImg = legacyImg;
    result.currentImg = currentImg;

    return result;
  }

  /**
   * 단일 이미지를 로드합니다.
   * @param {string} src - 이미지 경로
   * @returns {Promise<HTMLImageElement>}
   */
  async loadSingleImage(src) {
    return this.loadImage(src);
  }
}

// 전역 인스턴스
const comparator = new ImageComparator();
