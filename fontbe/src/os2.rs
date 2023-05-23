//! Generates a [OS/2](https://learn.microsoft.com/en-us/typography/opentype/spec/os2) table.

use fontdrasil::orchestration::Work;
use fontir::ir::GlobalMetricsInstance;
use read_fonts::{tables::hmtx::Hmtx, types::Tag, FontData, TopLevelTable};
use write_fonts::{tables::os2::Os2, OtRound};

use crate::{
    error::Error,
    orchestration::{BeWork, Context},
};

struct Os2Work {}

pub fn create_os2_work() -> Box<BeWork> {
    Box::new(Os2Work {})
}

/// <https://github.com/fonttools/fonttools/blob/115275cbf429d91b75ac5536f5f0b2d6fe9d823a/Lib/fontTools/ttLib/tables/O_S_2f_2.py#L336-L348>
fn x_avg_char_width(context: &Context) -> Result<i16, Error> {
    let static_metadata = context.ir.get_final_static_metadata();
    let hhea = context.get_hhea();
    let raw_hmtx = context.get_hmtx();
    let num_glyphs = static_metadata.glyph_order.len() as u64;
    let hmtx = Hmtx::read(
        FontData::new(raw_hmtx.get()),
        hhea.number_of_long_metrics,
        num_glyphs as u16,
    )
    .map_err(|_| Error::InvalidTableBytes(Hmtx::TAG))?;

    // count width > 0 only, including adding tail only if > 0
    let (count, total) = hmtx
        .h_metrics()
        .iter()
        .filter_map(|metric| match metric.advance() {
            0 => None,
            v => Some(v as u64),
        })
        .fold((0_u64, 0_u64), |(count, total), value| {
            (count + 1, total + value)
        });
    // plus any copies of the final advance
    let last_advance = hmtx
        .h_metrics()
        .last()
        .map(|m| m.advance() as u64)
        .unwrap_or_default();
    let (count, total) = if last_advance > 0 {
        let num_short = num_glyphs - hhea.number_of_long_metrics as u64;
        (count + num_short, total + num_short * last_advance)
    } else {
        (count, total)
    };

    Ok((total as f32 / count as f32).ot_round())
}

fn build_os2(x_avg_char_width: i16, vendor_id: Tag, metrics: &GlobalMetricsInstance) -> Os2 {
    Os2 {
        ach_vend_id: vendor_id,

        x_avg_char_width,

        s_cap_height: Some(metrics.cap_height.ot_round()),
        sx_height: Some(metrics.x_height.ot_round()),

        // Avoid "field must be present for version 2"
        ul_code_page_range_1: Some(0),
        ul_code_page_range_2: Some(0),
        us_default_char: Some(0),
        us_break_char: Some(0),
        us_max_context: Some(0),

        ..Default::default()
    }
}

impl Work<Context, Error> for Os2Work {
    /// Generate [OS/2](https://learn.microsoft.com/en-us/typography/opentype/spec/os2)
    fn exec(&self, context: &Context) -> Result<(), Error> {
        let static_metadata = context.ir.get_final_static_metadata();
        let metrics = context
            .ir
            .get_global_metrics()
            .at(static_metadata.default_location());

        context.set_os2(build_os2(
            x_avg_char_width(context)?,
            static_metadata.vendor_id,
            &metrics,
        ));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use fontir::{
        coords::NormalizedLocation,
        ir::{GlobalMetric, GlobalMetrics},
    };
    use read_fonts::types::Tag;

    use super::build_os2;

    #[test]
    fn build_basic_os2() {
        let default_location = NormalizedLocation::new();
        let mut global_metrics = GlobalMetrics::new(default_location.clone(), 1000);

        global_metrics.set(GlobalMetric::CapHeight, default_location.clone(), 37.5);
        global_metrics.set(GlobalMetric::XHeight, default_location.clone(), 112.2);

        let os2 = build_os2(42, Tag::new(b"DUCK"), &global_metrics.at(&default_location));

        assert_eq!(Tag::new(b"DUCK"), os2.ach_vend_id);
        assert_eq!(42, os2.x_avg_char_width);
        assert_eq!(Some(38), os2.s_cap_height);
        assert_eq!(Some(112), os2.sx_height);
    }
}
