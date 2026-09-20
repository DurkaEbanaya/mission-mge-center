/* src/render.rs
 *
 * Copyright 2026 Mission Center Developers
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * SPDX-License-Identifier: GPL-3.0-or-later
 */

use adw::gdk;
use gtk::gsk::{FillRule, PathBuilder, Stroke};
use gtk::prelude::SnapshotExt;
use gtk::Snapshot;

use mc_graph_core::dataset::DatasetGroup;
use mc_graph_core::scaling::{FillingSettings, ScalingSettings};

use crate::widget::GraphWidget;

pub fn plot_group(
    group: &DatasetGroup,
    snapshot: &Snapshot,
    width: f32,
    height: f32,
    parent: &GraphWidget,
) {
    let Some(dataset_points) = group.geometry(width, height, parent.point_spacing_factor()) else {
        return;
    };

    let settings = &group.dataset_settings;

    let color = parent.base_color();

    let stroke_color = gdk::RGBA::new(color.red(), color.green(), color.blue(), 1.);
    let fill_color = gdk::RGBA::new(color.red(), color.green(), color.blue(), settings.opacity);

    let stroke = Stroke::new(1.);

    if settings.dashed {
        stroke.set_dash(&[5., 5.]);
    }

    for (set_index, set) in dataset_points.iter().enumerate() {
        let set_split = set.split(|set| set.y.is_nan());

        for set in set_split {
            let path_builder = PathBuilder::new();

            let (Some(first_point), Some(last_point)) = (set.first(), set.last()) else {
                continue;
            };

            path_builder.move_to(first_point.x, first_point.y);

            let (mut lastx, mut lasty) = (first_point.x, first_point.y);

            for point in set.iter().skip(1) {
                if parent.smooth_graphs() {
                    let deltax = point.x - lastx;
                    path_builder.cubic_to(
                        lastx + deltax / 2f32,
                        lasty,
                        lastx + deltax / 2f32,
                        point.y,
                        point.x,
                        point.y,
                    );

                    lastx = point.x;
                    lasty = point.y;
                } else {
                    path_builder.line_to(point.x, point.y);
                }
            }

            if settings.vertical_dropoff_lines {
                match settings.fill {
                    FillingSettings::FillToBottom | FillingSettings::None => {
                        path_builder.line_to(last_point.x, height);
                        path_builder.line_to(first_point.x, height);
                    }
                    FillingSettings::FillToTop => {
                        path_builder.line_to(last_point.x, 0.);
                        path_builder.line_to(first_point.x, 0.);
                    }
                    FillingSettings::FillToZero => {
                        if settings.low_watermark >= 0.
                            || settings.low_watermark >= settings.high_watermark
                        {
                            path_builder.line_to(last_point.x, height);
                            path_builder.line_to(first_point.x, height);
                        } else {
                            let zeroheight = height * (settings.high_watermark)
                                / (settings.high_watermark - settings.low_watermark);
                            path_builder.line_to(last_point.x, zeroheight);
                            path_builder.line_to(first_point.x, zeroheight);
                        }
                    }
                }

                path_builder.close();

                let path = path_builder.to_path();

                if settings.fill != FillingSettings::None
                    && (settings.scaling_settings != ScalingSettings::Stacking
                        || set_index == dataset_points.len() - 1)
                {
                    snapshot.append_fill(&path, FillRule::Winding, &fill_color);
                }

                snapshot.append_stroke(&path, &stroke, &stroke_color);
            } else {
                let line = path_builder.to_path();

                let path_builder = PathBuilder::new();

                path_builder.move_to(first_point.x, first_point.y);

                // builder.add_path(&line) doesn't work with the fill

                let (mut lastx, mut lasty) = (first_point.x, first_point.y);

                for point in set.iter().skip(1) {
                    if parent.smooth_graphs() {
                        let deltax = point.x - lastx;
                        path_builder.cubic_to(
                            lastx + deltax / 2f32,
                            lasty,
                            lastx + deltax / 2f32,
                            point.y,
                            point.x,
                            point.y,
                        );

                        lastx = point.x;
                        lasty = point.y;
                    } else {
                        path_builder.line_to(point.x, point.y);
                    }
                }

                match settings.fill {
                    FillingSettings::FillToBottom | FillingSettings::None => {
                        path_builder.line_to(last_point.x, height);
                        path_builder.line_to(first_point.x, height);
                    }
                    FillingSettings::FillToTop => {
                        path_builder.line_to(last_point.x, 0.);
                        path_builder.line_to(first_point.x, 0.);
                    }
                    FillingSettings::FillToZero => {
                        if settings.low_watermark >= 0.
                            || settings.low_watermark >= settings.high_watermark
                        {
                            path_builder.line_to(last_point.x, height);
                            path_builder.line_to(first_point.x, height);
                        } else {
                            let zeroheight = height * (settings.high_watermark)
                                / (settings.high_watermark - settings.low_watermark);
                            path_builder.line_to(last_point.x, zeroheight);
                            path_builder.line_to(first_point.x, zeroheight);
                        }
                    }
                }

                path_builder.close();

                let path = path_builder.to_path();

                if settings.fill != FillingSettings::None
                    && (settings.scaling_settings != ScalingSettings::Stacking
                        || set_index == dataset_points.len() - 1)
                {
                    snapshot.append_fill(&path, FillRule::Winding, &fill_color);
                }

                snapshot.append_stroke(&line, &stroke, &stroke_color);
            }
        }
    }
}
