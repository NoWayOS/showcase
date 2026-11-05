// The Screen Compiler gallery, 10 of 11: every icon by its Lucide name, drawn in the colour of the text
// around it; pictures converted at build time, covering, contained and at their own size; and a
// progress bar the application fills.
// Each example is captioned with the TSX that made it. Written for the showcase in TSX and
// converted at build time like every other screen here; the launcher's Screen Compiler
// tile opens the first, and Back and Next go round them all.
import { Button, Column, HeaderBar, Icon, Image, ProgressBar, Row, Screen, Spacer, Text } from "nowayos:kit";

const GAP = 8;
const EDGE = 16;
const TIGHT = 8;

type GalleryProps = {
  onHome: () => void;
  onBack: () => void;
  onNext: () => void;
  done: number;
};

export function GalleryIconsScreen(props: GalleryProps) {
  return (
    <Screen>
      <Column gap={GAP} padding={EDGE}>
        <HeaderBar gap={GAP} padding={TIGHT}>
          <Button id="home" variant="secondary" onPress={props.onHome}>
            Home
          </Button>
          <Text id="title" role="heading">
            Icons, pictures and progress
          </Text>
          <Spacer />
          <Text>
            10 of 11
          </Text>
          <Button variant="secondary" onPress={props.onBack}>
            Back
          </Button>
          <Button id="next" onPress={props.onNext}>
            Next
          </Button>
        </HeaderBar>
        <Row gap={EDGE}>
          <Column gap={4}>
            <Icon name="clipboard-list" />
            <Text role="caption">clipboard-list</Text>
          </Column>
          <Column gap={4}>
            <Icon name="fuel" />
            <Text role="caption">fuel</Text>
          </Column>
          <Column gap={4}>
            <Icon name="house" />
            <Text role="caption">house</Text>
          </Column>
          <Column gap={4}>
            <Icon name="message-square" />
            <Text role="caption">message-square</Text>
          </Column>
          <Column gap={4}>
            <Icon name="shield-check" />
            <Text role="caption">shield-check</Text>
          </Column>
          <Column gap={4}>
            <Icon name="user-check" />
            <Text role="caption">user-check</Text>
          </Column>
        </Row>
        <Text role="caption">{'<Icon name="fuel" />'}</Text>
        <Row gap={EDGE} style={{ alignItems: "start" }}>
          <Image
            src="landscape"
            alt="A lake at sunset, hills behind it"
            fit="cover"
            style={{ width: 240, height: 80 }}
          />
          <Image src="landscape" alt="The same lake, whole" fit="contain" style={{ width: 80, height: 80 }} />
          <Image src="badge" alt="" fit="fill" />
          <Image src="badge" alt="" style={{ width: 96, height: 48, objectFit: "contain" }} />
        </Row>
        <Text role="caption">{'<Image src="landscape" alt="A lake at sunset" fit="cover" style={{ width: 240, height: 80 }} />'}</Text>
        <ProgressBar value={props.done} />
        <Text role="caption">{'<ProgressBar value={props.done} />: three quarters done'}</Text>
      </Column>
    </Screen>
  );
}
