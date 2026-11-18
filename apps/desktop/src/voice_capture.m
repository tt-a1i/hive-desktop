#import <AVFoundation/AVFoundation.h>
#import <Foundation/Foundation.h>
#import <Speech/Speech.h>
#include <stdio.h>
#include <string.h>

static int emit(NSDictionary *result) {
    NSData *data = [NSJSONSerialization dataWithJSONObject:result options:0 error:NULL];
    if (!data) return 1;
    fwrite(data.bytes, 1, data.length, stdout);
    fputc('\n', stdout);
    fflush(stdout);
    return 0;
}

static NSDictionary *failure(NSString *code) {
    return @{@"state": @"error", @"code": code};
}

static void pump(void) {
    @autoreleasepool {
        [[NSRunLoop currentRunLoop] runMode:NSDefaultRunLoopMode beforeDate:[NSDate dateWithTimeIntervalSinceNow:0.04]];
    }
}

int hive_voice_capture(const char *file, const char *locale, int statusOnly) {
    @autoreleasepool {
        if (@available(macOS 10.15, *)) {
            NSLocale *language = locale ? [[NSLocale alloc] initWithLocaleIdentifier:@(locale)] : NSLocale.currentLocale;
            SFSpeechRecognizer *recognizer = [[SFSpeechRecognizer alloc] initWithLocale:language];
            const SFSpeechRecognizerAuthorizationStatus speech = SFSpeechRecognizer.authorizationStatus;
            const AVAuthorizationStatus mic = [AVCaptureDevice authorizationStatusForMediaType:AVMediaTypeAudio];
            if (statusOnly) {
                return emit(@{@"onDevice": @(recognizer.supportsOnDeviceRecognition),
                              @"available": @(recognizer.available),
                              @"speechAuthorization": @(speech), @"microphoneAuthorization": @(mic)});
            }
            char start[16] = {0};
            if (!fgets(start, sizeof(start), stdin) || strcmp(start, "start\n") != 0) return 0;
            if (!recognizer.supportsOnDeviceRecognition || !recognizer.available) {
                return emit(failure(@"unavailable"));
            }

            __block BOOL finish = NO;
            __block BOOL cancelled = NO;
            dispatch_async(dispatch_get_global_queue(QOS_CLASS_UTILITY, 0), ^{
                char command[16] = {0};
                const BOOL stop = fgets(command, sizeof(command), stdin) && strcmp(command, "finish\n") == 0;
                dispatch_async(dispatch_get_main_queue(), ^{ finish = stop; cancelled = !stop; });
            });

            if (!file) {
                NSBundle *bundle = NSBundle.mainBundle;
                if (![bundle objectForInfoDictionaryKey:@"NSSpeechRecognitionUsageDescription"]
                    || ![bundle objectForInfoDictionaryKey:@"NSMicrophoneUsageDescription"]) {
                    return emit(failure(@"bundle_required"));
                }
                __block SFSpeechRecognizerAuthorizationStatus grantedSpeech = speech;
                if (speech == SFSpeechRecognizerAuthorizationStatusNotDetermined) {
                    [SFSpeechRecognizer requestAuthorization:^(SFSpeechRecognizerAuthorizationStatus value) {
                        dispatch_async(dispatch_get_main_queue(), ^{ grantedSpeech = value; });
                    }];
                    while (!finish && !cancelled && grantedSpeech == SFSpeechRecognizerAuthorizationStatusNotDetermined) pump();
                }
                if (finish || cancelled) return emit(failure(@"no_speech"));
                if (grantedSpeech != SFSpeechRecognizerAuthorizationStatusAuthorized) {
                    return emit(failure(@"speech_denied"));
                }

                __block AVAuthorizationStatus grantedMic = mic;
                if (mic == AVAuthorizationStatusNotDetermined) {
                    [AVCaptureDevice requestAccessForMediaType:AVMediaTypeAudio completionHandler:^(BOOL allowed) {
                        dispatch_async(dispatch_get_main_queue(), ^{
                            grantedMic = allowed ? AVAuthorizationStatusAuthorized : AVAuthorizationStatusDenied;
                        });
                    }];
                    while (!finish && !cancelled && grantedMic == AVAuthorizationStatusNotDetermined) pump();
                }
                if (finish || cancelled) return emit(failure(@"no_speech"));
                if (grantedMic != AVAuthorizationStatusAuthorized) return emit(failure(@"mic_denied"));
            }

            SFSpeechRecognitionRequest *request;
            AVAudioEngine *engine = nil;
            if (file) {
                request = [[SFSpeechURLRecognitionRequest alloc] initWithURL:[NSURL fileURLWithPath:@(file)]];
            } else {
                request = [[SFSpeechAudioBufferRecognitionRequest alloc] init];
                engine = [[AVAudioEngine alloc] init];
            }
            request.requiresOnDeviceRecognition = YES;
            request.shouldReportPartialResults = YES;
            request.taskHint = SFSpeechRecognitionTaskHintDictation;
            recognizer.queue = NSOperationQueue.mainQueue;
            __block NSString *latest = nil;
            __block BOOL final = NO;
            __block BOOL recognitionFailed = NO;
            SFSpeechRecognitionTask *task = [recognizer recognitionTaskWithRequest:request resultHandler:^(SFSpeechRecognitionResult *result, NSError *error) {
                if (result) latest = result.bestTranscription.formattedString;
                if (error) recognitionFailed = YES;
                if (error || result.isFinal) final = YES;
            }];
            if (engine) {
                AVAudioInputNode *input = engine.inputNode;
                AVAudioFormat *format = [input outputFormatForBus:0];
                if (!format.channelCount || format.sampleRate <= 0) {
                    [task cancel];
                    return emit(failure(@"mic_unavailable"));
                }
                SFSpeechAudioBufferRecognitionRequest *microphone = (SFSpeechAudioBufferRecognitionRequest *)request;
                [input installTapOnBus:0 bufferSize:1024 format:format block:^(AVAudioPCMBuffer *buffer, AVAudioTime *time) {
                    [microphone appendAudioPCMBuffer:buffer];
                }];
                if (![engine startAndReturnError:NULL]) {
                    [input removeTapOnBus:0];
                    [task cancel];
                    return emit(failure(@"mic_unavailable"));
                }
            }
            NSDate *deadline = [NSDate dateWithTimeIntervalSinceNow:60];
            while (!finish && !cancelled && !final && deadline.timeIntervalSinceNow > 0) pump();
            if (engine) {
                [engine stop];
                [engine.inputNode removeTapOnBus:0];
                [(SFSpeechAudioBufferRecognitionRequest *)request endAudio];
            }
            if (cancelled) {
                [task cancel];
                return 0;
            }
            if (!final && engine) {
                NSDate *settle = [NSDate dateWithTimeIntervalSinceNow:2];
                while (!final && !cancelled && settle.timeIntervalSinceNow > 0) pump();
            }
            [task cancel];
            if (cancelled) return 0;
            if (latest.length) return emit(@{@"state": @"completed", @"text": latest});
            return emit(failure(recognitionFailed ? @"recognition_failed" : @"no_speech"));
        }
        return statusOnly ? emit(@{@"onDevice": @NO, @"available": @NO}) : emit(failure(@"unavailable"));
    }
}
