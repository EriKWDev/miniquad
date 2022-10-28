//! MacOs implementation is basically a mix between
//! sokol_app's objective C code and Makepad's (https://github.com/makepad/makepad/blob/live/platform/src/platform/apple)
//! platform implementation
//!
use {
    crate::{
        event::{EventHandler, MouseButton},
        native::{
            apple::{
                apple_util::{self, *},
                frameworks::{self, *},
            },
            NativeDisplayData,
        },
        Context, CursorIcon, GraphicsContext,
    },
    std::{collections::HashMap, os::raw::c_void},
};

struct IosDisplay {}
impl crate::native::NativeDisplay for IosDisplay {
    fn screen_size(&self) -> (f32, f32) {
        (640., 800.)
    }
    fn dpi_scale(&self) -> f32 {
        1.
    }
    fn high_dpi(&self) -> bool {
        false
    }
    fn order_quit(&mut self) {}
    fn request_quit(&mut self) {}
    fn cancel_quit(&mut self) {}

    fn set_cursor_grab(&mut self, _grab: bool) {}
    fn show_mouse(&mut self, _show: bool) {}
    fn set_mouse_cursor(&mut self, _cursor: crate::CursorIcon) {}
    fn set_window_size(&mut self, _new_width: u32, _new_height: u32) {}
    fn set_fullscreen(&mut self, _fullscreen: bool) {}
    fn clipboard_get(&mut self) -> Option<String> {
        None
    }
    fn clipboard_set(&mut self, _data: &str) {}
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

struct WindowPayload {
    display: IosDisplay,
    context: Option<GraphicsContext>,
    event_handler: Option<Box<dyn EventHandler>>,
    f: Option<Box<dyn 'static + FnOnce(&mut crate::Context) -> Box<dyn EventHandler>>>,
}
impl WindowPayload {
    pub fn context(&mut self) -> Option<(&mut Context, &mut dyn EventHandler)> {
        let a = self.context.as_mut()?;
        let event_handler = self.event_handler.as_deref_mut()?;

        Some((a.with_display(&mut self.display), event_handler))
    }
}

fn get_window_payload(this: &Object) -> &mut WindowPayload {
    unsafe {
        let ptr: *mut c_void = *this.get_ivar("display_ptr");
        &mut *(ptr as *mut WindowPayload)
    }
}

pub fn define_glk_view() -> *const Class {
    let superclass = class!(GLKView);
    let mut decl = ClassDecl::new("QuadView", superclass).unwrap();

    unsafe {
        decl.add_method(sel!(isOpaque), yes as extern "C" fn(&Object, Sel) -> BOOL);
    }

    return decl.register();
}

pub fn define_glk_view_dlg() -> *const Class {
    let superclass = class!(NSObject);
    let mut decl = ClassDecl::new("QuadViewDlg", superclass).unwrap();

    extern "C" fn draw_in_rect(this: &Object, _: Sel, _: ObjcId, _: ObjcId) {
        let payload = get_window_payload(this);
        if payload.event_handler.is_none() {
            let f = payload.f.take().unwrap();
            payload.context = Some(GraphicsContext::new());
            payload.event_handler = Some(f(payload
                .context
                .as_mut()
                .unwrap()
                .with_display(&mut payload.display)));
        }

        if let Some((context, event_handler)) = payload.context() {
            event_handler.update(context);
            event_handler.draw(context);
        }

    }

    unsafe {
        decl.add_method(
            sel!(glkView: drawInRect:),
            draw_in_rect as extern "C" fn(&Object, Sel, ObjcId, ObjcId),
        );
    }
    decl.add_ivar::<*mut c_void>("display_ptr");
    return decl.register();
}

pub fn define_app_delegate() -> *const Class {
    let superclass = class!(NSObject);
    let mut decl = ClassDecl::new("NSAppDelegate", superclass).unwrap();

    extern "C" fn did_finish_launching_with_options(
        this: &Object,
        _: Sel,
        _: ObjcId,
        _: ObjcId,
    ) -> BOOL {
        unsafe {
            let main_screen: ObjcId = msg_send![class!(UIScreen), mainScreen];
            let screen_rect: NSRect = msg_send![main_screen, bounds];

            let window_obj: ObjcId = msg_send![class!(UIWindow), alloc];
            let window_obj: ObjcId = msg_send![window_obj, initWithFrame: screen_rect];

            let eagl_context_obj: ObjcId = msg_send![class!(EAGLContext), alloc];
            let mut eagl_context_obj: ObjcId = msg_send![eagl_context_obj, initWithAPI: 3];
            if eagl_context_obj.is_null() {
                eagl_context_obj = msg_send![eagl_context_obj, initWithAPI: 2];
                // capabilities.gles2
            }

            let glk_view_dlg_obj: ObjcId = msg_send![define_glk_view_dlg(), alloc];
            let glk_view_dlg_obj: ObjcId = msg_send![glk_view_dlg_obj, init];

            let glk_view_obj: ObjcId = msg_send![define_glk_view(), alloc];
            let glk_view_obj: ObjcId = msg_send![glk_view_obj, initWithFrame: screen_rect];

            let f = INIT_F.take().unwrap();
            let payload = Box::new(WindowPayload {
                display: IosDisplay {},
                f: Some(Box::new(f)),
                event_handler: None,
                context: None,
            });

            (*glk_view_dlg_obj).set_ivar(
                "display_ptr",
                Box::into_raw(payload) as *mut std::ffi::c_void,
            );
            let _: () = msg_send![
                glk_view_obj,
                setDrawableColorFormat: frameworks::GLKViewDrawableColorFormatRGBA8888
            ];
            let _: () = msg_send![
                glk_view_obj,
                setDrawableDepthFormat: frameworks::GLKViewDrawableDepthFormat::Format24 as i32
            ];
            let _: () = msg_send![
                glk_view_obj,
                setDrawableStencilFormat: frameworks::GLKViewDrawableStencilFormat::FormatNone
                    as i32
            ];
            // _sapp_view_obj.drawableMultisample = GLKViewDrawableMultisampleNone; /* FIXME */
            let _: () = msg_send![glk_view_obj, setContext: eagl_context_obj];
            let _: () = msg_send![glk_view_obj, setDelegate: glk_view_dlg_obj];
            let _: () = msg_send![glk_view_obj, setEnableSetNeedsDisplay: NO];
            let _: () = msg_send![glk_view_obj, setUserInteractionEnabled: YES];
            let _: () = msg_send![glk_view_obj, setMultipleTouchEnabled: YES];
            // if (_sapp.desc.high_dpi) {
            //     _sapp_view_obj.contentScaleFactor = 2.0;
            // }
            // else {
            //     _sapp_view_obj.contentScaleFactor = 1.0;
            // }
            let _: () = msg_send![window_obj, addSubview: glk_view_obj];

            let view_ctrl_obj: ObjcId = msg_send![class!(GLKViewController), alloc];
            let view_ctrl_obj: ObjcId = msg_send![view_ctrl_obj, init];

            let _: () = msg_send![view_ctrl_obj, setView: glk_view_obj];
            let _: () = msg_send![view_ctrl_obj, setPreferredFramesPerSecond:60];
            let _: () = msg_send![window_obj, setRootViewController: view_ctrl_obj];

            let _: () = msg_send![window_obj, makeKeyAndVisible];
        }
        1
    }

    unsafe {
        decl.add_method(
            sel!(application: didFinishLaunchingWithOptions:),
            did_finish_launching_with_options
                as extern "C" fn(&Object, Sel, ObjcId, ObjcId) -> BOOL,
        );
    }

    return decl.register();
}

// this is the way to pass argument to UiApplicationMain
// this static will be used exactly once, to .take() the "f" argument of "run"
static mut INIT_F: Option<Box<dyn FnOnce(&mut crate::Context) -> Box<dyn EventHandler>>> = None;

pub unsafe fn run<F>(conf: crate::conf::Conf, f: F)
where
    F: 'static + FnOnce(&mut crate::Context) -> Box<dyn EventHandler>,
{
    INIT_F = Some(Box::new(f));

    std::panic::set_hook(Box::new(|info| {
        let nsstring = apple_util::str_to_nsstring(&format!("{:?}", info));
        let _: () = frameworks::NSLog(nsstring);
    }));

    let argc = 1;
    let mut argv = b"Miniquad\0" as *const u8 as *mut i8;

    let class: ObjcId = msg_send!(define_app_delegate(), class);
    let class_string = frameworks::NSStringFromClass(class as _);

    UIApplicationMain(argc, &mut argv, nil, class_string);
}
