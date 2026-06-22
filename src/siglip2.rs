use image::DynamicImage;
use ndarray::Array;
use ort::{
    session::{
        Session,
        builder::{GraphOptimizationLevel, SessionBuilder},
    },
    value::Value,
};
use std::{
    path::Path,
};
use std::arch::x86_64::*;

pub struct Siglip2Hasher {
    session: Session,
}

impl Siglip2Hasher {
    /*
     * Take model here https://huggingface.co/onnx-community/siglip2-base-patch16-224-ONNX/blob/main/onnx/vision_model_q4.onnx
     */
    pub fn new(model_path: &Path) -> Result<Self, anyhow::Error> {
        let session = SessionBuilder::new()?
            .with_optimization_level(GraphOptimizationLevel::Level3).map_err(|_|anyhow::anyhow!("Failed to set GraphOptimizationLevel"))?
            .with_intra_threads(4).map_err(|_|anyhow::anyhow!("Failed to set intra with_intra_threads"))?
            .commit_from_file(model_path /*"./16vision_model_q4.onnx"*/).map_err(|_|anyhow::anyhow!("Failed to set intra with_intra_threads"))?;
        Ok(Self { session })
    }

    pub fn calculate_hash(&mut self, image: &DynamicImage) -> Result<Vec<f32>, anyhow::Error> {
        let height: u32 = 224;
        let width: u32 = 224;
        let resized =
            image::imageops::resize(image, height, width, image::imageops::FilterType::Nearest);
        let input_tensor: Vec<f32> = resized
            .pixels()
            .flat_map(|p| {
                let [r, g, b, _] = p.0;
                // нормализация в float 0.0–1.0
                vec![r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0]
            })
            .collect();

        let input_array =
            Array::from_shape_vec((1, 3, height as usize, width as usize), input_tensor)?;
        let input_value = Value::from_array(input_array)?;
        let outputs = self.session.run(vec![("pixel_values", &input_value)])?;
        let mut embedding: Vec<f32> = outputs[0]
            .try_extract_array()?
            .to_owned()
            .into_raw_vec_and_offset()
            .0;

        l2_normalize(&mut embedding);

        Ok(embedding)
    }

    pub fn calculate_similarity(&self, embedding1: &[f32], embedding2: &[f32]) -> f32 {
        cosine_similarity_normalized(embedding1, embedding2)
    }
}

#[target_feature(enable = "avx2,fma")]
pub unsafe fn dot_avx2(a: &[f32], b: &[f32]) -> f32 {
    let len = a.len();
    let ptr_a = a.as_ptr();
    let ptr_b = b.as_ptr();

    let mut acc0 = _mm256_setzero_ps();
    let mut acc1 = _mm256_setzero_ps();
    let mut acc2 = _mm256_setzero_ps();
    let mut acc3 = _mm256_setzero_ps();

    let chunks = len / 32; // 32 floats per loop (4x256-bit)

    for i in 0..chunks {
        let base = i * 32;

        let a0 = _mm256_loadu_ps(ptr_a.add(base));
        let b0 = _mm256_loadu_ps(ptr_b.add(base));

        let a1 = _mm256_loadu_ps(ptr_a.add(base + 8));
        let b1 = _mm256_loadu_ps(ptr_b.add(base + 8));

        let a2 = _mm256_loadu_ps(ptr_a.add(base + 16));
        let b2 = _mm256_loadu_ps(ptr_b.add(base + 16));

        let a3 = _mm256_loadu_ps(ptr_a.add(base + 24));
        let b3 = _mm256_loadu_ps(ptr_b.add(base + 24));

        acc0 = _mm256_fmadd_ps(a0, b0, acc0);
        acc1 = _mm256_fmadd_ps(a1, b1, acc1);
        acc2 = _mm256_fmadd_ps(a2, b2, acc2);
        acc3 = _mm256_fmadd_ps(a3, b3, acc3);
    }

    let mut tmp = [0.0f32; 8];

    let mut sum = 0.0f32;

    _mm256_storeu_ps(tmp.as_mut_ptr(), acc0);
    sum += tmp.iter().sum::<f32>();

    _mm256_storeu_ps(tmp.as_mut_ptr(), acc1);
    sum += tmp.iter().sum::<f32>();

    _mm256_storeu_ps(tmp.as_mut_ptr(), acc2);
    sum += tmp.iter().sum::<f32>();

    _mm256_storeu_ps(tmp.as_mut_ptr(), acc3);
    sum += tmp.iter().sum::<f32>();

    // tail
    let start = chunks * 32;
    for i in start..len {
        sum += ptr_a.add(i).read() * ptr_b.add(i).read();
    }

    sum
}


pub fn cosine_similarity_normalized(a: &[f32], b: &[f32]) -> f32 {
    assert_eq!(a.len(), b.len());

    if is_x86_feature_detected!("avx2") {
        unsafe { dot_avx2(a, b) }
    } else {
        a.iter().zip(b).map(|(&x, &y)| x * y).sum()
    }
}

pub fn l2_normalize(v: &mut [f32]) {
    let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    for x in v {
        *x /= norm;
    }
}